use std::{collections::HashMap, fs, path::Path, path::PathBuf, sync::Arc};

use cfghelper::cfghelper as cfg;
use handlebars::Handlebars;
use rdm4lib::{gltf_export, rdm_anim::RDAnim, rdm_material::RDMaterial, RDModell};
use serde::{Deserialize, Serialize};
use warp::{hyper::Uri, path::Tail, Filter};

struct WithTemplate<T: Serialize> {
    name: &'static str,
    value: T,
}

#[derive(Serialize, Deserialize, Debug)]
struct Test {
    user: i32,
}

fn render<T>(template: WithTemplate<T>, hbs: Arc<Handlebars>) -> impl warp::Reply
where
    T: Serialize,
{
    let render = hbs
        .render(template.name, &template.value)
        .unwrap_or_else(|err| err.to_string());
    warp::reply::html(render)
}

#[tokio::main]
async fn main() {
    static TEMPLATE: &'static str = include_str!("template.html");

    let mut hb = Handlebars::new();
    // register the template
    hb.register_template_string("template.html", TEMPLATE)
        .unwrap();

    // Turn Handlebars instance into a Filter so we can combine it
    // easily with others...
    let hb = Arc::new(hb);

    // Create a reusable closure to render template
    let handlebars = move |with_template| render(with_template, hb.clone());

    //GET /
    let route_form = warp::path("view")
        .and(warp::path::tail())
        .map(move |tail: Tail| {
            let l = test_cfg(tail.as_str());
            let mut cfg = None;
            let mut dir = None;
            match l {
                Kind::Cfg(c) => cfg = c,
                Kind::Dir(d) => dir = d,
            }

            //dbg!(&cfg);
            WithTemplate {
                name: "template.html",
                value: RenderInput {
                    cfg,
                    dir,
                    url: tail.as_str().to_string(),
                },
            }
        })
        .map(handlebars);

    let route_conv = warp::path("send")
        .and(warp::path::tail())
        .and(warp::body::content_length_limit(1024 * 32))
        .and(warp::body::form())
        .map(|tail: Tail, simple_map: HashMap<String, String>| {
            dbg!("Got a urlencoded body! {:#?}", &simple_map);
            read_body(tail.as_str(), simple_map);
            warp::redirect(format!("/view/{}", tail.as_str()).parse::<Uri>().unwrap())
        });

    let folder_gltf_route =
        warp::path("gltf").and(warp::fs::dir(r"C:\Users\lukas\Desktop\anno7\gltf"));

    let routes = warp::get()
        .and(route_form)
        .or(warp::post().and(route_conv).or(folder_gltf_route));

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

fn test_cfg(tail: &str) -> Kind {
    let path = Path::new(r"C:\Users\lukas\Desktop\anno7\");
    let path2 = path.join(tail);
    println!("path2 {:?}", path2);
    let mut vec = Vec::with_capacity(16);
    if path2.is_dir() {
        for entry in path2.read_dir().expect("read_dir call failed") {
            if let Ok(entry) = entry {
                let e = entry.path();
                let p = e.file_name().unwrap().to_str().unwrap();
                vec.push(p.to_string());
            }
        }
        return Kind::Dir(Some(vec));
    } else if path2.is_file() {
        let t = try_load(&path2);
        return Kind::Cfg(t);
    }
    panic!();
}

fn try_load(path: &Path) -> Option<cfg::AnnoCfg> {
    if path.is_file() && path.extension().unwrap().to_str().unwrap() == "cfg" {
        let cfg: cfg::AnnoCfg = cfg::parse_cfg(path).unwrap();
        return Some(cfg);
    }
    None
}

enum Kind {
    Cfg(Option<cfg::AnnoCfg>),
    Dir(Option<Vec<String>>),
}

#[derive(Serialize, Deserialize, Debug)]
struct RenderInput {
    cfg: Option<cfg::AnnoCfg>,
    dir: Option<Vec<String>>,
    url: String,
}

fn read_body(tail: &str, in_map: HashMap<String, String>) {
    let optcfg = test_cfg(tail);
    let path = Path::new(r"C:\Users\lukas\Desktop\anno7\");

    match optcfg {
        Kind::Cfg(ocfg) => {
            let cfg = ocfg.unwrap();
            let model_idx = in_map
                .get("modelId")
                .unwrap_or(&String::from("0"))
                .parse::<usize>()
                .unwrap();
            let model = &cfg.models.models_vec[model_idx];
            let rdm_rel_path = &model.file_name;
            let rdm_path = path.join(Path::new(rdm_rel_path));

            let mut rdm = RDModell::from(rdm_path.as_path());

            let mut diff_texs = Vec::new();
            let mut i = 0;
            loop {
                let diff_checked = in_map.contains_key(&i.to_string());
                if !diff_checked || model.materials.materials_vec.len() < i {
                    break;
                } else {
                    // patch psd to dds and append _0 to file name
                    let psd_dds = format!(
                        "{}{}.dds",
                        &model.materials.materials_vec[i].cModelDiffTex
                            [0..&model.materials.materials_vec[i].cModelDiffTex.len() - 4],
                        "_0"
                    );
                    let rel = PathBuf::from(&psd_dds);
                    diff_texs.push(path.join(rel));
                    dbg!(&diff_texs[i]);
                    i += 1;
                }
            }
            rdm.mat = Some(RDMaterial {
                c_model_diff_tex: diff_texs,
            });

            if let Some(anim_sel) = in_map.get("anim") {
                // TODO check if anim is actually is part of the cfg
                if anim_sel != "" {
                    let anim_path = path.join(anim_sel);
                    let anim = RDAnim::from(anim_path.as_path());
                    rdm.add_skin();
                    rdm.add_anim(anim)
                }
            }

            let dest_dir = path.join("gltf").join(tail);
            fs::create_dir_all(&dest_dir).unwrap();
            gltf_export::build(rdm, Some(dest_dir.clone()));
        }
        Kind::Dir(_) => panic!(),
    }
}
