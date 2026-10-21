//! Interactive, hex-editor-style terminal view.

use std::io::{self, IsTerminal, Stdout, Write};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
    self, disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::{execute, queue};

use crate::{output, Analysis, Block, BlockId, Edge, EdgeDirection, Endian};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum View {
    Hex,
    Inventory,
    Graph,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InventoryRow {
    Block(BlockId),
    Edge(usize),
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen, ResetColor);
    }
}

struct App {
    cursor: usize,
    scroll_row: usize,
    inventory_selection: usize,
    inventory_scroll: usize,
    inventory_expanded: Vec<bool>,
    graph_scroll: usize,
    view: View,
    history: Vec<usize>,
    show_boundaries: bool,
}

/// Run the interactive viewer. The caller must provide a terminal on stdout.
pub fn run(analysis: &Analysis, file_name: &str, initial_offset: usize) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "interactive mode requires a terminal; use --dump, --graph, or --dot",
        ));
    }
    let _guard = TerminalGuard::enter()?;
    let mut stdout = io::stdout();
    let mut app = App {
        cursor: initial_offset.min(analysis.bytes.len().saturating_sub(1)),
        scroll_row: 0,
        inventory_selection: 0,
        inventory_scroll: 0,
        inventory_expanded: vec![false; analysis.blocks.len()],
        graph_scroll: 0,
        view: View::Hex,
        history: Vec::new(),
        show_boundaries: false,
    };

    loop {
        let (width, height) = terminal::size()?;
        draw(&mut stdout, analysis, file_name, &mut app, width, height)?;
        match event::read()? {
            Event::Key(key)
                if key.kind == KeyEventKind::Press
                    && handle_key(key, analysis, &mut app, width, height) =>
            {
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

fn handle_key(key: KeyEvent, analysis: &Analysis, app: &mut App, width: u16, height: u16) -> bool {
    if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        || key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        return true;
    }
    if key.code == KeyCode::Char('v') {
        if app.view == View::Inventory {
            app.view = View::Hex;
        } else {
            sync_inventory_to_cursor(analysis, app);
            app.view = View::Inventory;
        }
        return false;
    }
    if matches!(key.code, KeyCode::Char('g') | KeyCode::Char('G'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        app.view = match app.view {
            View::Graph => View::Hex,
            View::Hex | View::Inventory => View::Graph,
        };
        return false;
    }
    if key.code == KeyCode::Char('g') && !key.modifiers.contains(KeyModifiers::ALT) {
        app.view = View::Hex;
        app.cursor = 0;
        app.scroll_row = 0;
        return false;
    }
    if key.code == KeyCode::Char('G') && !key.modifiers.contains(KeyModifiers::ALT) {
        app.view = View::Hex;
        app.cursor = analysis.bytes.len().saturating_sub(1);
        keep_cursor_visible(app, bytes_per_row(width), height);
        return false;
    }

    if app.view == View::Inventory {
        handle_inventory_key(key, analysis, app, height);
        return false;
    }

    if app.view == View::Graph {
        let page = height.saturating_sub(4) as usize;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                app.graph_scroll = app.graph_scroll.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') => app.graph_scroll += 1,
            KeyCode::PageUp => app.graph_scroll = app.graph_scroll.saturating_sub(page),
            KeyCode::PageDown => app.graph_scroll += page,
            KeyCode::Home => app.graph_scroll = 0,
            _ => {}
        }
        return false;
    }

    let bytes_per_row = bytes_per_row(width);
    let page = height.saturating_sub(5) as usize * bytes_per_row;
    let old_cursor = app.cursor;
    match key.code {
        KeyCode::BackTab => jump_next_record_boundary(analysis, app),
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
            jump_next_record_boundary(analysis, app)
        }
        KeyCode::Tab => jump_next_outgoing_in_record(analysis, app),
        KeyCode::Left | KeyCode::Char('h') => app.cursor = app.cursor.saturating_sub(1),
        KeyCode::Right | KeyCode::Char('l') => app.cursor = app.cursor.saturating_add(1),
        KeyCode::Up | KeyCode::Char('k') => app.cursor = app.cursor.saturating_sub(bytes_per_row),
        KeyCode::Down | KeyCode::Char('j') => app.cursor = app.cursor.saturating_add(bytes_per_row),
        KeyCode::PageUp => app.cursor = app.cursor.saturating_sub(page),
        KeyCode::PageDown => app.cursor = app.cursor.saturating_add(page),
        KeyCode::Home => app.cursor = 0,
        KeyCode::End => app.cursor = analysis.bytes.len().saturating_sub(1),
        KeyCode::Char('[') => jump_block(analysis, app, false),
        KeyCode::Char(']') => jump_block(analysis, app, true),
        KeyCode::Char('b') => app.show_boundaries = !app.show_boundaries,
        KeyCode::Char('i') => jump_to_incoming(analysis, app),
        KeyCode::Enter => {
            if let Some(edge) = analysis.edge_at(app.cursor) {
                app.history.push(app.cursor);
                app.cursor = analysis.blocks[edge.target_block.index()].payload_offset;
            }
        }
        KeyCode::Backspace => {
            if let Some(offset) = app.history.pop() {
                app.cursor = offset;
            }
        }
        _ => {}
    }
    app.cursor = app.cursor.min(analysis.bytes.len().saturating_sub(1));
    if app.cursor != old_cursor {
        keep_cursor_visible(app, bytes_per_row, height);
    }
    false
}

fn handle_inventory_key(key: KeyEvent, analysis: &Analysis, app: &mut App, height: u16) {
    let rows = inventory_rows(analysis, &app.inventory_expanded);
    app.inventory_selection = app.inventory_selection.min(rows.len().saturating_sub(1));
    let page = height.saturating_sub(5).max(1) as usize;
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.inventory_selection = app.inventory_selection.saturating_sub(1)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.inventory_selection =
                (app.inventory_selection + 1).min(rows.len().saturating_sub(1))
        }
        KeyCode::PageUp => app.inventory_selection = app.inventory_selection.saturating_sub(page),
        KeyCode::PageDown => {
            app.inventory_selection =
                (app.inventory_selection + page).min(rows.len().saturating_sub(1))
        }
        KeyCode::Home => app.inventory_selection = 0,
        KeyCode::End => app.inventory_selection = rows.len().saturating_sub(1),
        KeyCode::Right | KeyCode::Char('l') => {
            if let Some(InventoryRow::Block(block_id)) = rows.get(app.inventory_selection) {
                if inventory_block_expandable(analysis, *block_id) {
                    app.inventory_expanded[block_id.index()] = true;
                }
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            collapse_inventory_row(analysis, app, &rows);
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            activate_inventory_row(analysis, app, &rows, key.code == KeyCode::Enter);
        }
        _ => {}
    }
    keep_inventory_selection_visible(app, height);
}

fn activate_inventory_row(
    analysis: &Analysis,
    app: &mut App,
    rows: &[InventoryRow],
    open_non_expandable: bool,
) {
    let Some(row) = rows.get(app.inventory_selection).copied() else {
        return;
    };
    match row {
        InventoryRow::Block(block_id) if inventory_block_expandable(analysis, block_id) => {
            app.inventory_expanded[block_id.index()] = !app.inventory_expanded[block_id.index()];
        }
        InventoryRow::Block(block_id) if open_non_expandable => {
            app.cursor = analysis.blocks[block_id.index()].payload_offset;
            app.view = View::Hex;
        }
        InventoryRow::Edge(edge_index) if open_non_expandable => {
            app.history.push(app.cursor);
            app.cursor = analysis.edges[edge_index].file_offset;
            app.view = View::Hex;
        }
        InventoryRow::Block(_) | InventoryRow::Edge(_) => {}
    }
}

fn collapse_inventory_row(analysis: &Analysis, app: &mut App, rows: &[InventoryRow]) {
    let Some(row) = rows.get(app.inventory_selection).copied() else {
        return;
    };
    let block_id = match row {
        InventoryRow::Block(block_id) => block_id,
        InventoryRow::Edge(edge_index) => analysis.edges[edge_index].source_block,
    };
    if !app.inventory_expanded[block_id.index()] {
        return;
    }
    app.inventory_expanded[block_id.index()] = false;
    app.inventory_selection = inventory_rows(analysis, &app.inventory_expanded)
        .iter()
        .position(|row| *row == InventoryRow::Block(block_id))
        .unwrap();
}

fn sync_inventory_to_cursor(analysis: &Analysis, app: &mut App) {
    let Some(block_id) = analysis.block_at(app.cursor).map(|block| block.id) else {
        return;
    };
    let rows = inventory_rows(analysis, &app.inventory_expanded);
    if let Some(selection) = rows
        .iter()
        .position(|row| *row == InventoryRow::Block(block_id))
    {
        app.inventory_selection = selection;
    }
}

fn inventory_rows(analysis: &Analysis, expanded: &[bool]) -> Vec<InventoryRow> {
    let mut rows = Vec::with_capacity(analysis.blocks.len());
    for block in &analysis.blocks {
        rows.push(InventoryRow::Block(block.id));
        if expanded.get(block.id.index()).copied().unwrap_or(false)
            && inventory_block_expandable(analysis, block.id)
        {
            rows.extend(
                analysis
                    .edges
                    .iter()
                    .enumerate()
                    .filter(|(_, edge)| edge.source_block == block.id)
                    .map(|(edge_index, _)| InventoryRow::Edge(edge_index)),
            );
        }
    }
    rows
}

fn inventory_block_expandable(analysis: &Analysis, block_id: BlockId) -> bool {
    analysis
        .edges
        .iter()
        .any(|edge| edge.source_block == block_id)
}

fn keep_inventory_selection_visible(app: &mut App, height: u16) {
    let visible_rows = height.saturating_sub(4).max(1) as usize;
    if app.inventory_selection < app.inventory_scroll {
        app.inventory_scroll = app.inventory_selection;
    } else if app.inventory_selection >= app.inventory_scroll + visible_rows {
        app.inventory_scroll = app.inventory_selection + 1 - visible_rows;
    }
}

fn jump_block(analysis: &Analysis, app: &mut App, next: bool) {
    let current = analysis
        .block_at(app.cursor)
        .map(|block| block.id.index())
        .unwrap_or(0);
    let target = if next {
        (current + 1).min(analysis.blocks.len() - 1)
    } else {
        current.saturating_sub(1)
    };
    app.cursor = analysis.blocks[target].payload_offset;
}

fn jump_to_incoming(analysis: &Analysis, app: &mut App) {
    let Some(block) = analysis.block_at(app.cursor) else {
        return;
    };
    let Some(edge) = first_forward_incoming(analysis, block.id) else {
        return;
    };
    app.history.push(app.cursor);
    app.cursor = edge.file_offset;
}

fn first_forward_incoming(analysis: &Analysis, block_id: BlockId) -> Option<&crate::Edge> {
    analysis
        .edges
        .iter()
        .find(|edge| edge.target_block == block_id && edge.direction == EdgeDirection::Forward)
}

fn record_at(analysis: &Analysis, offset: usize) -> Option<(&Block, u32)> {
    let block = analysis.block_at(offset)?;
    let record_index = if offset < block.payload_offset {
        0
    } else {
        ((offset - block.payload_offset) / block.stride as usize) as u32
    };
    Some((block, record_index.min(block.count - 1)))
}

fn record_outgoing_edges(
    analysis: &Analysis,
    block_id: BlockId,
    record_index: u32,
) -> impl Iterator<Item = &Edge> {
    analysis
        .edges
        .iter()
        .filter(move |edge| edge.source_block == block_id && edge.record_index == record_index)
}

fn jump_next_outgoing_in_record(analysis: &Analysis, app: &mut App) {
    let Some((block, record_index)) = record_at(analysis, app.cursor) else {
        return;
    };
    let current_edge = analysis
        .edge_at(app.cursor)
        .filter(|edge| edge.source_block == block.id && edge.record_index == record_index);
    let after = current_edge
        .map(|edge| edge.file_offset)
        .unwrap_or_else(|| app.cursor.saturating_sub(1));
    let next = record_outgoing_edges(analysis, block.id, record_index)
        .filter(|edge| edge.file_offset > after)
        .min_by_key(|edge| edge.file_offset)
        .or_else(|| {
            record_outgoing_edges(analysis, block.id, record_index)
                .min_by_key(|edge| edge.file_offset)
        });
    if let Some(edge) = next {
        app.cursor = edge.file_offset;
    }
}

fn jump_next_record_boundary(analysis: &Analysis, app: &mut App) {
    let Some((block, record_index)) = record_at(analysis, app.cursor) else {
        app.cursor = analysis.blocks[0].payload_offset;
        return;
    };
    if app.cursor < block.payload_offset {
        app.cursor = block.payload_offset;
    } else if record_index + 1 < block.count {
        app.cursor = block.payload_offset + (record_index as usize + 1) * block.stride as usize;
    } else if let Some(next) = analysis.blocks.get(block.id.index() + 1) {
        app.cursor = next.payload_offset;
    }
}

fn keep_cursor_visible(app: &mut App, bytes_per_row: usize, height: u16) {
    let visible_rows = height.saturating_sub(5).max(1) as usize;
    let cursor_row = app.cursor / bytes_per_row;
    if cursor_row < app.scroll_row {
        app.scroll_row = cursor_row;
    } else if cursor_row >= app.scroll_row + visible_rows {
        app.scroll_row = cursor_row + 1 - visible_rows;
    }
}

fn bytes_per_row(width: u16) -> usize {
    if width >= 78 {
        16
    } else if width >= 48 {
        8
    } else {
        4
    }
}

const STRIDE_COLOR: Color = Color::Magenta;
const TEXT_COLOR: Color = Color::Yellow;
const OFFSET_COLOR_A: Color = Color::Green;
const OFFSET_COLOR_B: Color = Color::Cyan;
const OFFSET_BACKGROUND_A: Color = Color::Rgb { r: 0, g: 38, b: 24 };
const OFFSET_BACKGROUND_B: Color = Color::Rgb { r: 0, g: 30, b: 42 };
const DESTINATION_BACKGROUND: Color = Color::DarkGreen;
const SOURCE_BACKGROUND: Color = Color::Rgb {
    r: 72,
    g: 24,
    b: 78,
};
const COUNT_BACKGROUND: Color = Color::Rgb { r: 0, g: 48, b: 58 };
const STRIDE_BACKGROUND: Color = Color::Rgb {
    r: 52,
    g: 20,
    b: 68,
};
const RECORD_BACKGROUND_A: Color = Color::Rgb {
    r: 12,
    g: 25,
    b: 46,
};
const RECORD_BACKGROUND_B: Color = Color::Rgb {
    r: 10,
    g: 37,
    b: 36,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ByteStyle {
    foreground: Color,
    background: Option<Color>,
    attribute: Option<Attribute>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HighlightContext {
    boundary_block: Option<BlockId>,
    destination: Option<(usize, usize)>,
    incoming: Option<(usize, usize)>,
}

impl HighlightContext {
    fn new(analysis: &Analysis, app: &App) -> Self {
        let boundary_block = if app.show_boundaries {
            analysis.block_at(app.cursor).map(|block| block.id)
        } else {
            None
        };
        let destination = analysis.edge_at(app.cursor).map(|edge| {
            let target = &analysis.blocks[edge.target_block.index()];
            (
                target.payload_offset,
                (target.payload_offset + 4).min(target.payload_end),
            )
        });
        let incoming = analysis
            .block_at(app.cursor)
            .and_then(|block| first_forward_incoming(analysis, block.id))
            .map(|edge| (edge.file_offset, edge.file_offset + 4));
        Self {
            boundary_block,
            destination,
            incoming,
        }
    }
}

fn draw(
    stdout: &mut Stdout,
    analysis: &Analysis,
    file_name: &str,
    app: &mut App,
    width: u16,
    height: u16,
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    let title = format!(
        " rdm-inspect | {file_name} | {} bytes | {} | {} blocks ",
        analysis.bytes.len(),
        analysis.endian,
        analysis.blocks.len()
    );
    queue!(
        stdout,
        SetBackgroundColor(Color::DarkBlue),
        SetForegroundColor(Color::White),
        SetAttribute(Attribute::Bold)
    )?;
    print_at(stdout, 0, 0, width, &title)?;
    let usable_width = width.saturating_sub(1);
    let fill = usable_width.saturating_sub(title.chars().count() as u16);
    if fill != 0 {
        queue!(stdout, Print(" ".repeat(fill as usize)))?;
    }
    queue!(stdout, ResetColor, SetAttribute(Attribute::Reset))?;

    match app.view {
        View::Hex => draw_hex(stdout, analysis, app, width, height)?,
        View::Inventory => draw_inventory(stdout, analysis, app, width, height)?,
        View::Graph => draw_graph(stdout, analysis, app, width, height)?,
    }
    stdout.flush()
}

fn draw_hex(
    stdout: &mut Stdout,
    analysis: &Analysis,
    app: &mut App,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let bytes_per_row = bytes_per_row(width);
    keep_cursor_visible(app, bytes_per_row, height);
    let visible_rows = height.saturating_sub(5) as usize;
    let hex_width = 10 + bytes_per_row * 3;
    let main_width = hex_width + bytes_per_row + 3;
    let show_details = width as usize >= main_width + 30;
    let highlights = HighlightContext::new(analysis, app);

    let columns = (0..bytes_per_row)
        .map(|column| format!("{column:02x} "))
        .collect::<String>();
    print_at(stdout, 0, 1, width, &format!("  offset   {columns} ASCII"))?;

    for screen_row in 0..visible_rows {
        let row = app.scroll_row + screen_row;
        let start = row * bytes_per_row;
        if start >= analysis.bytes.len() {
            break;
        }
        let y = screen_row as u16 + 2;
        let marker = row_marker(analysis, &highlights, start, bytes_per_row);
        let address_color = match marker {
            '>' => Color::Green,
            '<' => Color::Magenta,
            '[' | ']' | '#' => Color::Magenta,
            _ if app.cursor / bytes_per_row == row => Color::White,
            _ => Color::DarkGrey,
        };
        queue!(
            stdout,
            MoveTo(0, y),
            SetForegroundColor(address_color),
            Print(format!("{marker}{start:08x} ")),
            ResetColor
        )?;
        for column in 0..bytes_per_row {
            let offset = start + column;
            if offset >= analysis.bytes.len() {
                queue!(stdout, Print("   "))?;
                continue;
            }
            let style = byte_style(analysis, &highlights, offset);
            apply_byte_style(stdout, style, offset == app.cursor)?;
            queue!(
                stdout,
                Print(format!("{:02x} ", analysis.bytes[offset])),
                ResetColor,
                SetAttribute(Attribute::Reset)
            )?;
        }
        queue!(stdout, Print(" "))?;
        for column in 0..bytes_per_row {
            let offset = start + column;
            if offset >= analysis.bytes.len() {
                break;
            }
            let byte = analysis.bytes[offset];
            let character = if byte.is_ascii_graphic() || byte == b' ' {
                byte as char
            } else {
                '.'
            };
            let style = byte_style(analysis, &highlights, offset);
            apply_byte_style(stdout, style, offset == app.cursor)?;
            queue!(
                stdout,
                Print(character),
                ResetColor,
                SetAttribute(Attribute::Reset)
            )?;
        }
    }

    if show_details {
        draw_details(
            stdout,
            analysis,
            app.cursor,
            app.show_boundaries,
            main_width as u16,
            width,
            height,
        )?;
    }

    if height >= 2 {
        draw_legend(stdout, height - 2, width, app.show_boundaries)?;
        let help = "Tab outgoing  Shift+Tab next-record  g/G file-ends  Ctrl+G graph  v inventory  i incoming  Enter follow  b boundaries  [] block  q quit";
        queue!(
            stdout,
            SetForegroundColor(Color::Black),
            SetBackgroundColor(Color::Grey)
        )?;
        print_at(stdout, 0, height - 1, width, help)?;
        let usable_width = width.saturating_sub(1);
        let fill =
            usable_width.saturating_sub(help.chars().count().min(usable_width as usize) as u16);
        queue!(stdout, Print(" ".repeat(fill as usize)), ResetColor)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct InventoryLayout {
    block_width: u16,
    meaning_x: u16,
    meaning_width: u16,
    payload_x: u16,
    count_x: u16,
    stride_x: u16,
    bytes_x: u16,
    incoming_x: u16,
    outgoing_x: u16,
}

impl InventoryLayout {
    fn new(width: u16) -> Self {
        let usable = width.saturating_sub(1);
        let block_width = 12;
        let meaning_x = block_width;
        let payload_x = usable.saturating_sub(49).max(meaning_x + 1);
        Self {
            block_width,
            meaning_x,
            meaning_width: payload_x.saturating_sub(meaning_x),
            payload_x,
            count_x: payload_x + 12,
            stride_x: payload_x + 21,
            bytes_x: payload_x + 30,
            incoming_x: payload_x + 40,
            outgoing_x: payload_x + 45,
        }
    }
}

fn draw_inventory(
    stdout: &mut Stdout,
    analysis: &Analysis,
    app: &mut App,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let rows = inventory_rows(analysis, &app.inventory_expanded);
    app.inventory_selection = app.inventory_selection.min(rows.len().saturating_sub(1));
    keep_inventory_selection_visible(app, height);
    let visible = height.saturating_sub(4).max(1) as usize;
    app.inventory_scroll = app.inventory_scroll.min(rows.len().saturating_sub(visible));
    let layout = InventoryLayout::new(width);

    queue!(
        stdout,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold)
    )?;
    print_at(stdout, 0, 1, width, " Linear block inventory")?;
    queue!(stdout, ResetColor, SetAttribute(Attribute::Reset))?;

    fill_inventory_row(
        stdout,
        2,
        width,
        Color::Rgb {
            r: 29,
            g: 36,
            b: 51,
        },
    )?;
    queue!(
        stdout,
        SetForegroundColor(Color::Grey),
        SetAttribute(Attribute::Bold)
    )?;
    draw_inventory_cell(stdout, 0, 2, layout.block_width, "Block", false)?;
    draw_inventory_cell(
        stdout,
        layout.meaning_x,
        2,
        layout.meaning_width,
        "Meaning",
        false,
    )?;
    draw_inventory_cell(stdout, layout.payload_x, 2, 12, "Payload", false)?;
    draw_inventory_cell(stdout, layout.count_x, 2, 9, "Count", true)?;
    draw_inventory_cell(stdout, layout.stride_x, 2, 9, "Stride", true)?;
    draw_inventory_cell(stdout, layout.bytes_x, 2, 10, "Bytes", true)?;
    draw_inventory_cell(stdout, layout.incoming_x, 2, 5, "In", true)?;
    draw_inventory_cell(stdout, layout.outgoing_x, 2, 4, "Out", true)?;
    queue!(stdout, ResetColor, SetAttribute(Attribute::Reset))?;

    for (screen_row, (row_index, row)) in rows
        .iter()
        .enumerate()
        .skip(app.inventory_scroll)
        .take(visible)
        .enumerate()
    {
        let y = screen_row as u16 + 3;
        let selected = row_index == app.inventory_selection;
        let background = if selected {
            Color::DarkBlue
        } else if row_index.is_multiple_of(2) {
            Color::Rgb {
                r: 21,
                g: 26,
                b: 36,
            }
        } else {
            Color::Rgb {
                r: 25,
                g: 31,
                b: 43,
            }
        };
        fill_inventory_row(stdout, y, width, background)?;
        if selected {
            queue!(
                stdout,
                SetForegroundColor(Color::White),
                SetAttribute(Attribute::Bold)
            )?;
        } else {
            queue!(stdout, SetForegroundColor(Color::Grey))?;
        }
        match row {
            InventoryRow::Block(block_id) => {
                draw_inventory_block(stdout, analysis, app, layout, *block_id, y, selected)?
            }
            InventoryRow::Edge(edge_index) => {
                draw_inventory_edge(stdout, analysis, layout, *edge_index, y, selected)?
            }
        }
        queue!(stdout, ResetColor, SetAttribute(Attribute::Reset))?;
    }

    if height != 0 {
        queue!(
            stdout,
            SetForegroundColor(Color::Black),
            SetBackgroundColor(Color::Grey)
        )?;
        print_at(
            stdout,
            0,
            height - 1,
            width,
            "F/B forward/backward-self candidates  up/down select  Enter/Space expand  right/left open/close  Enter edge->hex  g/G file-ends  Ctrl+G graph  v hex  q quit",
        )?;
        queue!(stdout, ResetColor)?;
    }
    Ok(())
}

fn draw_inventory_block(
    stdout: &mut Stdout,
    analysis: &Analysis,
    app: &App,
    layout: InventoryLayout,
    block_id: BlockId,
    y: u16,
    selected: bool,
) -> io::Result<()> {
    let block = &analysis.blocks[block_id.index()];
    let expandable = inventory_block_expandable(analysis, block_id);
    let block_cell =
        if block_id == BlockId::ROOT && expandable && app.inventory_expanded[block_id.index()] {
            "#000 root ▾".to_owned()
        } else if block_id == BlockId::ROOT && expandable {
            "#000 root ▸".to_owned()
        } else if block_id == BlockId::ROOT {
            "#000 root".to_owned()
        } else if expandable && app.inventory_expanded[block_id.index()] {
            format!("#{block_id:03} ▾")
        } else if expandable {
            format!("#{block_id:03} ▸")
        } else {
            format!("#{block_id:03}")
        };
    draw_inventory_cell(stdout, 0, y, layout.block_width, &block_cell, false)?;

    if !selected {
        let color = if block.text_preview(&analysis.bytes).is_some() {
            TEXT_COLOR
        } else if block_id == BlockId::ROOT {
            Color::Cyan
        } else {
            Color::White
        };
        queue!(stdout, SetForegroundColor(color))?;
    }
    let meaning =
        output::block_meaning(block, &analysis.bytes, layout.meaning_width.max(1) as usize);
    draw_inventory_cell(
        stdout,
        layout.meaning_x,
        y,
        layout.meaning_width,
        &meaning,
        false,
    )?;
    draw_inventory_cell(
        stdout,
        layout.payload_x,
        y,
        12,
        &format!("0x{:08x}", block.payload_offset),
        false,
    )?;
    draw_inventory_cell(stdout, layout.count_x, y, 9, &block.count.to_string(), true)?;
    draw_inventory_cell(
        stdout,
        layout.stride_x,
        y,
        9,
        &block.stride.to_string(),
        true,
    )?;
    draw_inventory_cell(
        stdout,
        layout.bytes_x,
        y,
        10,
        &block.payload_size().to_string(),
        true,
    )?;
    draw_inventory_cell(
        stdout,
        layout.incoming_x,
        y,
        5,
        &output::incoming_count(analysis, block_id).to_string(),
        true,
    )?;
    draw_inventory_cell(
        stdout,
        layout.outgoing_x,
        y,
        4,
        &output::outgoing_count(analysis, block_id).to_string(),
        true,
    )
}

fn draw_inventory_edge(
    stdout: &mut Stdout,
    analysis: &Analysis,
    layout: InventoryLayout,
    edge_index: usize,
    y: u16,
    selected: bool,
) -> io::Result<()> {
    let edge = &analysis.edges[edge_index];
    let target = &analysis.blocks[edge.target_block.index()];
    let direction_marker = if edge.direction == EdgeDirection::Forward {
        'F'
    } else {
        'B'
    };
    let block_cell = format!(
        "  {direction_marker} r{}→#{:03}",
        edge.record_index, edge.target_block
    );
    draw_inventory_cell(stdout, 0, y, layout.block_width, &block_cell, false)?;
    if !selected {
        queue!(
            stdout,
            SetForegroundColor(if edge.direction == EdgeDirection::Forward {
                Color::Green
            } else {
                Color::Yellow
            })
        )?;
    }
    let mut meaning = format!("+0x{:x} @0x{:08x}", edge.field_offset, edge.file_offset);
    if layout.meaning_width >= 30 {
        meaning.push_str(&format!(
            " → #{:03} ({})",
            edge.target_block, edge.direction
        ));
    }
    draw_inventory_cell(
        stdout,
        layout.meaning_x,
        y,
        layout.meaning_width,
        &meaning,
        false,
    )?;
    draw_inventory_cell(
        stdout,
        layout.payload_x,
        y,
        12,
        &format!("0x{:08x}", target.payload_offset),
        false,
    )
}

fn fill_inventory_row(stdout: &mut Stdout, y: u16, width: u16, color: Color) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(0, y),
        SetBackgroundColor(color),
        Print(" ".repeat(width.saturating_sub(1) as usize))
    )
}

fn draw_inventory_cell(
    stdout: &mut Stdout,
    x: u16,
    y: u16,
    width: u16,
    text: &str,
    right_aligned: bool,
) -> io::Result<()> {
    if width == 0 {
        return Ok(());
    }
    let clipped: String = text.chars().take(width as usize).collect();
    let value = if right_aligned {
        format!("{clipped:>width$}", width = width as usize)
    } else {
        clipped
    };
    queue!(stdout, MoveTo(x, y), Print(value))
}

fn draw_details(
    stdout: &mut Stdout,
    analysis: &Analysis,
    offset: usize,
    show_boundaries: bool,
    x: u16,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let pane_width = width.saturating_sub(x + 1);
    draw_section_title(
        stdout,
        x,
        2,
        pane_width,
        &format!("Selection / {}", analysis.endian),
        Color::Cyan,
        height,
    )?;
    draw_detail_line(
        stdout,
        x,
        3,
        pane_width,
        "file offset",
        &format!("0x{offset:08x}"),
        height,
    )?;
    draw_detail_line(
        stdout,
        x,
        4,
        pane_width,
        "region",
        &byte_description(analysis, offset),
        height,
    )?;
    draw_detail_line(
        stdout,
        x,
        5,
        pane_width,
        "u8",
        &format!(
            "{} / 0x{:02x}",
            analysis.bytes[offset], analysis.bytes[offset]
        ),
        height,
    )?;

    let u16_value = read_u16(analysis, offset);
    draw_detail_line(
        stdout,
        x,
        6,
        pane_width,
        "u16",
        &u16_value
            .map(|value| format!("{value} / 0x{value:04x}"))
            .unwrap_or_else(|| "-".to_owned()),
        height,
    )?;
    let u32_value = read_u32(analysis, offset);
    draw_detail_line(
        stdout,
        x,
        7,
        pane_width,
        "u32",
        &u32_value
            .map(|value| format!("{value} / 0x{value:08x}"))
            .unwrap_or_else(|| "-".to_owned()),
        height,
    )?;
    draw_detail_line(
        stdout,
        x,
        8,
        pane_width,
        "i32",
        &u32_value
            .map(|value| (value as i32).to_string())
            .unwrap_or_else(|| "-".to_owned()),
        height,
    )?;
    draw_detail_line(
        stdout,
        x,
        9,
        pane_width,
        "f32",
        &u32_value
            .map(|value| format!("{:e}", f32::from_bits(value)))
            .unwrap_or_else(|| "-".to_owned()),
        height,
    )?;

    let block = analysis.block_at(offset);
    let block_title = block
        .map(|block| {
            format!(
                "B{:03} {}{}",
                block.id,
                analysis.structural_label(block.id),
                if show_boundaries {
                    "  [boundaries]"
                } else {
                    ""
                }
            )
        })
        .unwrap_or_else(|| "No block".to_owned());
    draw_section_title(stdout, x, 11, pane_width, &block_title, Color::Cyan, height)?;

    let header = block
        .map(|block| format!("0x{:08x}", block.header_offset))
        .unwrap_or_else(|| "-".to_owned());
    let payload = block
        .map(|block| {
            format!(
                "0x{:08x}..0x{:08x}",
                block.payload_offset, block.payload_end
            )
        })
        .unwrap_or_else(|| "-".to_owned());
    let count = block
        .map(|block| block.count.to_string())
        .unwrap_or_else(|| "-".to_owned());
    let stride = block
        .map(|block| block.stride.to_string())
        .unwrap_or_else(|| "-".to_owned());
    let payload_size = block
        .map(|block| block.payload_size().to_string())
        .unwrap_or_else(|| "-".to_owned());
    let edges = block
        .map(|block| {
            format!(
                "{} in / {} out",
                analysis.forward_incoming_count(block.id),
                analysis.forward_outgoing_count(block.id)
            )
        })
        .unwrap_or_else(|| "-".to_owned());
    let location = block
        .map(|block| block_location(block, offset))
        .unwrap_or_else(|| "-".to_owned());
    let preview = block
        .and_then(|block| block.text_preview(&analysis.bytes))
        .map(|preview| format!("\"{preview}\""))
        .unwrap_or_else(|| "-".to_owned());
    let incoming = block
        .and_then(|block| first_forward_incoming(analysis, block.id))
        .map(|edge| {
            format!(
                "B{} record {} +0x{:x} @0x{:08x}  (i)",
                edge.source_block, edge.record_index, edge.field_offset, edge.file_offset
            )
        })
        .unwrap_or_else(|| "-".to_owned());
    for (y, label, value) in [
        (12, "header", header),
        (13, "payload", payload),
        (14, "count", count),
        (15, "stride", stride),
        (16, "payload size", payload_size),
        (17, "forward refs", edges),
        (18, "location", location),
        (19, "incoming ref", incoming),
        (20, "text", preview),
    ] {
        draw_detail_line(stdout, x, y, pane_width, label, &value, height)?;
    }

    draw_section_title(
        stdout,
        x,
        22,
        pane_width,
        "Offset field",
        Color::Green,
        height,
    )?;
    let edge = analysis.edge_at(offset);
    let target = edge
        .map(|edge| {
            let block = &analysis.blocks[edge.target_block.index()];
            format!("B{} payload 0x{:08x}", block.id, block.payload_offset)
        })
        .unwrap_or_else(|| "-".to_owned());
    let field = edge
        .map(|edge| format!("record {} +0x{:x}", edge.record_index, edge.field_offset))
        .unwrap_or_else(|| "-".to_owned());
    let direction = edge
        .map(|edge| format!("{}  (Enter follows)", edge.direction))
        .unwrap_or_else(|| "-".to_owned());
    for (y, label, value) in [
        (23, "target", target),
        (24, "field", field),
        (25, "direction", direction),
    ] {
        draw_detail_line(stdout, x, y, pane_width, label, &value, height)?;
    }
    draw_record_outgoing(stdout, analysis, offset, x, pane_width, height)?;
    Ok(())
}

fn draw_record_outgoing(
    stdout: &mut Stdout,
    analysis: &Analysis,
    offset: usize,
    x: u16,
    pane_width: u16,
    height: u16,
) -> io::Result<()> {
    let Some((block, record_index)) = record_at(analysis, offset) else {
        return Ok(());
    };
    let outgoing_count = record_outgoing_edges(analysis, block.id, record_index).count();
    draw_section_title(
        stdout,
        x,
        27,
        pane_width,
        &format!("Outgoing record {record_index} ({outgoing_count})"),
        Color::Green,
        height,
    )?;
    if height <= 30 {
        return Ok(());
    }
    queue!(stdout, SetForegroundColor(Color::DarkGrey))?;
    print_at(
        stdout,
        x,
        28,
        pane_width,
        "Tab next  i incoming  Enter follow  Shift+Tab record",
    )?;
    queue!(stdout, ResetColor)?;

    let available = height.saturating_sub(31) as usize;
    if available == 0 {
        return Ok(());
    }
    if outgoing_count == 0 {
        queue!(stdout, SetForegroundColor(Color::DarkGrey))?;
        print_at(stdout, x, 29, pane_width, "  (none)")?;
        return queue!(stdout, ResetColor);
    }

    let selected_offset = analysis.edge_at(offset).and_then(|edge| {
        (edge.source_block == block.id && edge.record_index == record_index)
            .then_some(edge.file_offset)
    });
    let selected_position = selected_offset.and_then(|selected| {
        record_outgoing_edges(analysis, block.id, record_index)
            .position(|edge| edge.file_offset == selected)
    });
    let start = if outgoing_count > available {
        selected_position
            .unwrap_or(0)
            .saturating_sub(available - 1)
            .min(outgoing_count - available)
    } else {
        0
    };
    for (row, edge) in record_outgoing_edges(analysis, block.id, record_index)
        .skip(start)
        .take(available)
        .enumerate()
    {
        let selected = selected_offset == Some(edge.file_offset);
        queue!(
            stdout,
            SetForegroundColor(if selected {
                Color::White
            } else if edge.direction == EdgeDirection::Forward {
                Color::Green
            } else {
                Color::Yellow
            })
        )?;
        if selected {
            queue!(stdout, SetAttribute(Attribute::Bold))?;
        }
        let target = analysis.structural_label(edge.target_block);
        print_at(
            stdout,
            x,
            29 + row as u16,
            pane_width,
            &format!(
                "{} +0x{:x} @0x{:08x} -> B{:03} {} [{}]",
                if selected { '>' } else { ' ' },
                edge.field_offset,
                edge.file_offset,
                edge.target_block,
                target,
                edge.direction
            ),
        )?;
        queue!(stdout, ResetColor, SetAttribute(Attribute::Reset))?;
    }
    Ok(())
}

fn draw_section_title(
    stdout: &mut Stdout,
    x: u16,
    y: u16,
    width: u16,
    title: &str,
    color: Color,
    height: u16,
) -> io::Result<()> {
    if y >= height.saturating_sub(2) {
        return Ok(());
    }
    queue!(
        stdout,
        SetAttribute(Attribute::Bold),
        SetForegroundColor(color)
    )?;
    print_at(stdout, x, y, width, title)?;
    queue!(stdout, ResetColor, SetAttribute(Attribute::Reset))
}

fn draw_detail_line(
    stdout: &mut Stdout,
    x: u16,
    y: u16,
    width: u16,
    label: &str,
    value: &str,
    height: u16,
) -> io::Result<()> {
    if y >= height.saturating_sub(2) {
        return Ok(());
    }
    print_at(stdout, x, y, width, &format!("{label:<13}{value}"))
}

fn block_location(block: &Block, offset: usize) -> String {
    if offset < block.header_offset + 4 {
        return format!("count header +0x{:x}", offset - block.header_offset);
    }
    if offset < block.payload_offset {
        return format!("stride header +0x{:x}", offset - block.header_offset - 4);
    }
    let relative = offset - block.payload_offset;
    format!(
        "record {} +0x{:x}",
        relative / block.stride as usize,
        relative % block.stride as usize
    )
}

fn draw_graph(
    stdout: &mut Stdout,
    analysis: &Analysis,
    app: &mut App,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let graph = output::graph(analysis);
    let lines: Vec<_> = graph.lines().collect();
    let visible = height.saturating_sub(3) as usize;
    app.graph_scroll = app.graph_scroll.min(lines.len().saturating_sub(visible));
    for (screen_row, line) in lines
        .iter()
        .skip(app.graph_scroll)
        .take(visible)
        .enumerate()
    {
        let color = if line.contains("field ") {
            Color::Green
        } else if line.contains("candidate") || line.contains("Not reached") {
            Color::Yellow
        } else {
            Color::Cyan
        };
        queue!(stdout, SetForegroundColor(color))?;
        print_at(stdout, 0, screen_row as u16 + 1, width, line)?;
        queue!(stdout, ResetColor)?;
    }
    if height != 0 {
        queue!(
            stdout,
            SetForegroundColor(Color::Black),
            SetBackgroundColor(Color::Grey)
        )?;
        print_at(
            stdout,
            0,
            height - 1,
            width,
            "up/down scroll  Ctrl+G hex  g/G file-ends  v inventory  q quit",
        )?;
        queue!(stdout, ResetColor)?;
    }
    Ok(())
}

fn row_marker(
    analysis: &Analysis,
    highlights: &HighlightContext,
    row_start: usize,
    bytes_per_row: usize,
) -> char {
    let row_end = row_start + bytes_per_row;
    if highlights
        .destination
        .is_some_and(|(start, _)| (row_start..row_end).contains(&start))
    {
        return '>';
    }
    if highlights
        .incoming
        .is_some_and(|(start, _)| (row_start..row_end).contains(&start))
    {
        return '<';
    }
    let Some(block_id) = highlights.boundary_block else {
        return ' ';
    };
    let block = &analysis.blocks[block_id.index()];
    let starts = (row_start..row_end).contains(&block.header_offset);
    let ends = (row_start..row_end).contains(&block.payload_end.saturating_sub(1));
    match (starts, ends) {
        (true, true) => '#',
        (true, false) => '[',
        (false, true) => ']',
        (false, false) => ' ',
    }
}

fn byte_style(analysis: &Analysis, highlights: &HighlightContext, offset: usize) -> ByteStyle {
    let mut style = ByteStyle {
        foreground: Color::White,
        background: None,
        attribute: None,
    };

    if offset < 4 {
        style.foreground = Color::Magenta;
    } else if (0x10..0x14).contains(&offset) {
        style.foreground = OFFSET_COLOR_A;
        style.background = Some(OFFSET_BACKGROUND_A);
        style.attribute = Some(Attribute::Bold);
    } else if offset < 0x14 {
        style.foreground = Color::DarkGrey;
    } else if let Some(block) = analysis.block_at(offset) {
        if offset < block.header_offset + 4 {
            style.foreground = Color::Cyan;
        } else if offset < block.payload_offset {
            style.foreground = STRIDE_COLOR;
        } else if block.text_preview(&analysis.bytes).is_some() {
            style.foreground = TEXT_COLOR;
        } else {
            style.foreground = if block.id.index() % 2 == 0 {
                Color::Blue
            } else {
                Color::DarkCyan
            };
        }
    }

    if let Some(block_id) = highlights.boundary_block {
        let block = &analysis.blocks[block_id.index()];
        if (block.header_offset..block.payload_end).contains(&offset) {
            if offset < block.header_offset + 4 {
                style.background = Some(COUNT_BACKGROUND);
            } else if offset < block.payload_offset {
                style.background = Some(STRIDE_BACKGROUND);
            } else {
                let relative = offset - block.payload_offset;
                let record_index = relative / block.stride as usize;
                style.background = Some(if record_index.is_multiple_of(2) {
                    RECORD_BACKGROUND_A
                } else {
                    RECORD_BACKGROUND_B
                });
                if relative.is_multiple_of(block.stride as usize) {
                    style.attribute = Some(Attribute::Underlined);
                }
            }
        }
    }

    if let Some(edge) = analysis.edge_at(offset) {
        let alternate = (edge.file_offset / 4) % 2 == 1;
        match edge.direction {
            EdgeDirection::Forward => {
                style.foreground = if alternate {
                    OFFSET_COLOR_B
                } else {
                    OFFSET_COLOR_A
                };
                style.background = Some(if alternate {
                    OFFSET_BACKGROUND_B
                } else {
                    OFFSET_BACKGROUND_A
                });
            }
            EdgeDirection::BackwardOrSelf => {
                style.foreground = if alternate {
                    Color::Magenta
                } else {
                    Color::Red
                };
                style.background = Some(Color::Rgb { r: 45, g: 8, b: 12 });
            }
        }
        style.attribute = Some(Attribute::Bold);
    }

    if highlights
        .incoming
        .is_some_and(|(start, end)| (start..end).contains(&offset))
    {
        style.background = Some(SOURCE_BACKGROUND);
        style.attribute = Some(Attribute::Underlined);
    }

    if highlights
        .destination
        .is_some_and(|(start, end)| (start..end).contains(&offset))
    {
        style.background = Some(DESTINATION_BACKGROUND);
        style.attribute = Some(Attribute::Underlined);
    }
    style
}

fn apply_byte_style(stdout: &mut Stdout, style: ByteStyle, selected: bool) -> io::Result<()> {
    queue!(stdout, SetForegroundColor(style.foreground))?;
    if let Some(background) = style.background {
        queue!(stdout, SetBackgroundColor(background))?;
    }
    if let Some(attribute) = style.attribute {
        queue!(stdout, SetAttribute(attribute))?;
    }
    if selected {
        queue!(
            stdout,
            SetBackgroundColor(Color::White),
            SetForegroundColor(Color::Black),
            SetAttribute(Attribute::Bold)
        )?;
    }
    Ok(())
}

fn draw_legend(stdout: &mut Stdout, y: u16, width: u16, show_boundaries: bool) -> io::Result<()> {
    if width < 62 {
        return print_at(
            stdout,
            0,
            y,
            width,
            if show_boundaries {
                "b boundaries: ON"
            } else {
                "b boundaries: off"
            },
        );
    }
    queue!(
        stdout,
        MoveTo(0, y),
        SetForegroundColor(Color::Cyan),
        Print("COUNT "),
        SetForegroundColor(STRIDE_COLOR),
        Print("STRIDE "),
        SetForegroundColor(TEXT_COLOR),
        Print("TEXT "),
        SetForegroundColor(OFFSET_COLOR_A),
        SetBackgroundColor(OFFSET_BACKGROUND_A),
        Print(" OFF-A "),
        ResetColor,
        SetForegroundColor(OFFSET_COLOR_B),
        SetBackgroundColor(OFFSET_BACKGROUND_B),
        Print(" OFF-B "),
        ResetColor,
        SetForegroundColor(Color::White),
        SetBackgroundColor(SOURCE_BACKGROUND),
        Print(" SRC "),
        ResetColor,
        SetForegroundColor(Color::White),
        SetBackgroundColor(DESTINATION_BACKGROUND),
        Print(" DST "),
        ResetColor,
        Print(if show_boundaries {
            "  BOUNDARIES:ON"
        } else {
            "  BOUNDARIES:off"
        })
    )
}

fn byte_description(analysis: &Analysis, offset: usize) -> String {
    if offset < 4 {
        return "RDM magic / byte-order marker".to_owned();
    }
    if let Some(edge) = analysis.edge_at(offset) {
        return format!("{} offset field", edge.direction);
    }
    if (0x10..0x14).contains(&offset) {
        return "root payload offset".to_owned();
    }
    if offset < 0x14 {
        return "preamble word".to_owned();
    }
    if let Some(block) = analysis.block_at(offset) {
        if offset < block.header_offset + 4 {
            return format!("B{} count", block.id);
        }
        if offset < block.payload_offset {
            return format!("B{} stride", block.id);
        }
        return format!("B{} record payload", block.id);
    }
    "unknown".to_owned()
}

fn read_u16(analysis: &Analysis, offset: usize) -> Option<u16> {
    let value: [u8; 2] = analysis.bytes.get(offset..offset + 2)?.try_into().unwrap();
    Some(match analysis.endian {
        Endian::Little => u16::from_le_bytes(value),
        Endian::Big => u16::from_be_bytes(value),
    })
}

fn read_u32(analysis: &Analysis, offset: usize) -> Option<u32> {
    let value: [u8; 4] = analysis.bytes.get(offset..offset + 4)?.try_into().unwrap();
    Some(match analysis.endian {
        Endian::Little => u32::from_le_bytes(value),
        Endian::Big => u32::from_be_bytes(value),
    })
}

fn print_at(stdout: &mut Stdout, x: u16, y: u16, width: u16, text: &str) -> io::Result<()> {
    let clipped: String = text
        .chars()
        .take(width.saturating_sub(1) as usize)
        .collect();
    queue!(stdout, MoveTo(x, y), Print(clipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    fn styled_analysis() -> Analysis {
        let mut bytes = b"RDM\x01".to_vec();
        for value in [0x14, 0, 4, 0x1c] {
            push_u32(&mut bytes, value);
        }

        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, 8);
        push_u32(&mut bytes, 44);
        push_u32(&mut bytes, 54);

        push_u32(&mut bytes, 2);
        push_u32(&mut bytes, 1);
        bytes.extend(b"hi");

        push_u32(&mut bytes, 2);
        push_u32(&mut bytes, 1);
        bytes.extend(b"ok");
        Analysis::parse(bytes).unwrap()
    }

    fn array_analysis() -> Analysis {
        let mut bytes = b"RDM\x01".to_vec();
        for value in [0x14, 0, 4, 0x1c] {
            push_u32(&mut bytes, value);
        }

        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, 4);
        push_u32(&mut bytes, 40);

        push_u32(&mut bytes, 2);
        push_u32(&mut bytes, 4);
        push_u32(&mut bytes, 56);
        push_u32(&mut bytes, 66);

        push_u32(&mut bytes, 2);
        push_u32(&mut bytes, 1);
        bytes.extend(b"hi");

        push_u32(&mut bytes, 2);
        push_u32(&mut bytes, 1);
        bytes.extend(b"ok");
        Analysis::parse(bytes).unwrap()
    }

    fn app(cursor: usize, show_boundaries: bool) -> App {
        App {
            cursor,
            scroll_row: 0,
            inventory_selection: 0,
            inventory_scroll: 0,
            inventory_expanded: vec![false; 3],
            graph_scroll: 0,
            view: View::Hex,
            history: Vec::new(),
            show_boundaries,
        }
    }

    #[test]
    fn row_width_adapts_to_small_terminals() {
        assert_eq!(bytes_per_row(100), 16);
        assert_eq!(bytes_per_row(60), 8);
        assert_eq!(bytes_per_row(40), 4);
    }

    #[test]
    fn g_keys_jump_file_ends_while_control_g_toggles_graph() {
        let analysis = styled_analysis();
        let mut state = app(44, false);

        assert!(!handle_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
            &analysis,
            &mut state,
            100,
            32,
        ));
        assert_eq!(state.cursor, 0);
        assert_eq!(state.view, View::Hex);

        state.view = View::Inventory;
        assert!(!handle_key(
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
            &analysis,
            &mut state,
            100,
            32,
        ));
        assert_eq!(state.cursor, analysis.bytes.len() - 1);
        assert_eq!(state.view, View::Hex);

        assert!(!handle_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            &analysis,
            &mut state,
            100,
            32,
        ));
        assert_eq!(state.view, View::Graph);
        assert!(!handle_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            &analysis,
            &mut state,
            100,
            32,
        ));
        assert_eq!(state.view, View::Hex);
    }

    #[test]
    fn count_stride_and_text_have_distinct_colors() {
        let analysis = styled_analysis();
        let highlights = HighlightContext::new(&analysis, &app(0, false));
        assert_eq!(
            byte_style(&analysis, &highlights, 20).foreground,
            Color::Cyan
        );
        assert_eq!(
            byte_style(&analysis, &highlights, 24).foreground,
            STRIDE_COLOR
        );
        assert_eq!(
            byte_style(&analysis, &highlights, 44).foreground,
            TEXT_COLOR
        );
        assert_ne!(STRIDE_COLOR, TEXT_COLOR);
    }

    #[test]
    fn adjacent_offset_fields_alternate_as_four_byte_groups() {
        let analysis = styled_analysis();
        let highlights = HighlightContext::new(&analysis, &app(0, false));
        let first = byte_style(&analysis, &highlights, 28);
        let first_end = byte_style(&analysis, &highlights, 31);
        let second = byte_style(&analysis, &highlights, 32);
        assert_eq!(first, first_end);
        assert_ne!(first.foreground, second.foreground);
        assert_ne!(first.background, second.background);
    }

    #[test]
    fn hovering_an_offset_marks_a_visible_destination() {
        let analysis = styled_analysis();
        let highlights = HighlightContext::new(&analysis, &app(28, false));
        assert_eq!(highlights.destination, Some((44, 46)));
        assert_eq!(
            byte_style(&analysis, &highlights, 44).background,
            Some(DESTINATION_BACKGROUND)
        );
        assert_eq!(row_marker(&analysis, &highlights, 32, 16), '>');
    }

    #[test]
    fn selected_block_exposes_and_jumps_to_its_incoming_field() {
        let analysis = styled_analysis();
        let highlights = HighlightContext::new(&analysis, &app(44, false));
        assert_eq!(highlights.incoming, Some((28, 32)));
        assert_eq!(
            byte_style(&analysis, &highlights, 28).background,
            Some(SOURCE_BACKGROUND)
        );
        assert_eq!(row_marker(&analysis, &highlights, 16, 16), '<');

        let mut state = app(44, false);
        assert!(!handle_key(
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
            &analysis,
            &mut state,
            100,
            32,
        ));
        assert_eq!(state.cursor, 28);
        assert_eq!(state.history, vec![44]);
        assert!(!handle_key(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &analysis,
            &mut state,
            100,
            32,
        ));
        assert_eq!(state.cursor, 44);
    }

    #[test]
    fn tab_selects_outgoing_and_shift_tab_advances_the_record() {
        let analysis = styled_analysis();
        let mut state = app(28, false);

        assert!(!handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &analysis,
            &mut state,
            100,
            32,
        ));
        assert_eq!(state.cursor, 32);

        assert!(!handle_key(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            &analysis,
            &mut state,
            100,
            32,
        ));
        assert_eq!(state.cursor, analysis.blocks[1].payload_offset);
    }

    #[test]
    fn shift_tab_advances_between_records_in_an_array() {
        let analysis = array_analysis();
        let mut state = app(analysis.blocks[1].payload_offset, false);
        state
            .inventory_expanded
            .resize(analysis.blocks.len(), false);

        assert_eq!(
            record_outgoing_edges(&analysis, BlockId::new(1), 0).count(),
            1,
            "record zero should expose its outgoing field"
        );
        assert!(!handle_key(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            &analysis,
            &mut state,
            100,
            32,
        ));
        assert_eq!(state.cursor, analysis.blocks[1].payload_offset + 4);
        assert_eq!(record_at(&analysis, state.cursor).unwrap().1, 1);

        let mut shifted_tab = app(analysis.blocks[1].payload_offset, false);
        shifted_tab
            .inventory_expanded
            .resize(analysis.blocks.len(), false);
        assert!(!handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT),
            &analysis,
            &mut shifted_tab,
            100,
            32,
        ));
        assert_eq!(shifted_tab.cursor, analysis.blocks[1].payload_offset + 4);
    }

    #[test]
    fn boundary_overlay_separates_headers_and_stride_one_records() {
        let analysis = styled_analysis();
        let highlights = HighlightContext::new(&analysis, &app(44, true));
        assert_eq!(
            byte_style(&analysis, &highlights, 36).background,
            Some(COUNT_BACKGROUND)
        );
        assert_eq!(
            byte_style(&analysis, &highlights, 40).background,
            Some(STRIDE_BACKGROUND)
        );
        assert_eq!(
            byte_style(&analysis, &highlights, 44).background,
            Some(RECORD_BACKGROUND_A)
        );
        assert_eq!(
            byte_style(&analysis, &highlights, 45).background,
            Some(RECORD_BACKGROUND_B)
        );
        assert_eq!(row_marker(&analysis, &highlights, 32, 16), '#');
    }

    #[test]
    fn inventory_expands_array_edges_and_opens_their_source() {
        let analysis = array_analysis();
        let mut state = app(0, false);
        state
            .inventory_expanded
            .resize(analysis.blocks.len(), false);
        state.view = View::Inventory;
        state.inventory_selection = 1;

        assert!(inventory_block_expandable(&analysis, BlockId::new(1)));
        assert!(inventory_block_expandable(&analysis, BlockId::new(0)));
        assert_eq!(
            inventory_rows(&analysis, &state.inventory_expanded),
            vec![
                InventoryRow::Block(BlockId::new(0)),
                InventoryRow::Block(BlockId::new(1)),
                InventoryRow::Block(BlockId::new(2)),
                InventoryRow::Block(BlockId::new(3)),
            ]
        );

        assert!(!handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &analysis,
            &mut state,
            120,
            20,
        ));
        assert!(state.inventory_expanded[1]);
        assert_eq!(
            inventory_rows(&analysis, &state.inventory_expanded),
            vec![
                InventoryRow::Block(BlockId::new(0)),
                InventoryRow::Block(BlockId::new(1)),
                InventoryRow::Edge(1),
                InventoryRow::Edge(2),
                InventoryRow::Block(BlockId::new(2)),
                InventoryRow::Block(BlockId::new(3)),
            ]
        );

        state.inventory_selection = 2;
        assert!(!handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &analysis,
            &mut state,
            120,
            20,
        ));
        assert_eq!(state.view, View::Hex);
        assert_eq!(state.cursor, analysis.edges[1].file_offset);
        assert_eq!(state.history, vec![0]);
    }

    #[test]
    fn inventory_exposes_every_graph_source_in_ark_waterfall() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rdm4lib/rdm/ark_waterfall2.rdm");
        let analysis = Analysis::parse(std::fs::read(fixture).unwrap()).unwrap();
        let expandable: Vec<_> = analysis
            .blocks
            .iter()
            .filter(|block| inventory_block_expandable(&analysis, block.id))
            .map(|block| block.id)
            .collect();

        assert_eq!(
            expandable,
            [0, 1, 4, 5, 7, 13, 14].map(BlockId::new).to_vec()
        );
        for edge in &analysis.edges {
            assert!(inventory_block_expandable(&analysis, edge.source_block));
        }
    }

    #[test]
    fn inventory_hotkey_selects_the_current_block() {
        let analysis = array_analysis();
        let mut state = app(analysis.blocks[2].payload_offset, false);
        state
            .inventory_expanded
            .resize(analysis.blocks.len(), false);

        assert!(!handle_key(
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
            &analysis,
            &mut state,
            120,
            20,
        ));
        assert_eq!(state.view, View::Inventory);
        assert_eq!(state.inventory_selection, 2);

        assert!(!handle_key(
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
            &analysis,
            &mut state,
            120,
            20,
        ));
        assert_eq!(state.view, View::Hex);
    }
}
