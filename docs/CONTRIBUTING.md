# Contributing

## Commit Message Convention

This project follows [Conventional Commits](https://www.conventionalcommits.org/),
inspired by the [Angular commit message guidelines](https://github.com/angular/angular/blob/main/contributing-docs/commit-message-guidelines.md).

### Format

```
<type>(<scope>): <description>
```

The `<type>` and `<description>` fields are mandatory, the `(<scope>)` field is
optional.

### Types

| Type         | Description                                             |
| ------------ | ------------------------------------------------------- |
| **feat**     | A new feature                                           |
| **fix**      | A bug fix                                               |
| **build**    | Changes to the build system or dependencies             |
| **docs**     | Documentation only changes                              |
| **style**    | Formatting, whitespace, etc. (no logic changes)         |
| **refactor** | Code change that neither fixes a bug nor adds a feature |
| **test**     | Adding missing tests or correcting existing tests       |
| **ci**       | Changes to CI configuration and scripts                 |
| **perf**     | A code change that improves performance                 |
| **chore**    | Other changes that don't modify src or test files       |

### Scopes

| Scope  | Description                   |
| ------ | ----------------------------- |
| `lib`  | Core library (`rdm4lib/`)     |
| `cli`  | CLI binary (`src/`)           |
| `ci`   | CI workflows (`.github/`)     |
| `nix`  | Nix flake and dev environment |
| `docs` | Documentation                 |

The scope may be omitted for changes that span multiple areas.

### Summary Style

- Use the imperative, present tense: "add" not "added" nor "adds"
- Do not capitalize the first letter
- No period (`.`) at the end

### Examples

- `feat(lib): add support for new vertex format`
- `fix(cli): correct output path handling on Windows`
- `ci(nix): add flake-checker policy job`
- `docs: update README installation steps`
- `style(lib): apply rustfmt to rdm_data_main`
- `refactor(lib): split gltf reader into submodules`
- `test(lib): add integration test for animation export`

### Merge Commits

Merge commits should follow the default Git merge message format:

```
Merge branch '<branch name>'
```

### Revert Commits

If the commit reverts a previous commit, it should begin with `revert:`,
followed by the header of the reverted commit. The body should contain the SHA
of the reverted commit:

```
revert: feat(lib): add support for new vertex format

This reverts commit <SHA>.
```

### Large Refactors

When performing large refactors that move files or change formatting across the
project, add the commit hash to `.git-blame-ignore-revs`. This ensures
`git blame` continues to show the original authors of the logic rather than the
refactor commit.

## Future

This project may adopt
[semantic-release](https://github.com/semantic-release/semantic-release) for
automated versioning and changelog generation. Following the commit conventions
above will make that transition seamless.

______________________________________________________________________

> Please ensure your code conforms to the project's formatting and linting
> rules. See [LINTER.md](./LINTER.md) for details.
