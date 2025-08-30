# gits-cli

```
Bulk git wrapper for multi-repo workspaces

Usage: gits [OPTIONS] [GIT_ARGS]...

Arguments:
  [GIT_ARGS]...  Git command and args (first token is the git subcommand)

Options:
      --root <ROOT>                    Search root (defaults to current directory)
      --absolute-path                  Print absolute headings instead of relative paths
      --parent                         Include ancestor repositories from cwd up to filesystem root
      --max-depth <MAX_DEPTH>          Limit child search depth (0 = only root). Omit for unlimited
      --list                           List discovered repositories without executing git
      --heading-style <HEADING_STYLE>  Heading style for repository separators [default: rule] [possible values: plain, rule]
      --color <COLOR>                  Color mode for headings [default: auto] [possible values: auto, always, never]
      --no-heading                     Suppress headings entirely (even for multiple repos)
  -h, --help                           Print help
  -V, --version                        Print version
```
