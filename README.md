# rsincron
![crates.io](https://img.shields.io/crates/v/rsincron.svg)

An attempt to resurrect `incron` but in rust.

## Installation
### Cargo
Run ```cargo install rsincron```.


## Usage
### `rsincrond`
Simply run the daemon with `rsincrond`. The program doesn't background itself.
Logs get written to the standard error. Customize targets and log levels with
the `RUST_LOG` environment variable (see the
[`env_logger` crate](https://docs.rs/env_logger/0.10.1/env_logger/#enabling-logging)).

### `rsincrontab`
Tool to manage your watches. Usage:
```bash
rsincrontab <mode>
```
where mode is one of `edit`, `list` or `remove`.

#### edit
Opens a temp file with your `$EDITOR` (if not found defaults to `/usr/bin/vi`)
to edit/add new rsincrons. The format used is:
```
<path-to-folder-or-file>  <MASKS,ATTRS>  <command-to-execute ARGS>
```
you can use either spaces or tabs to separate the fields. Lines starting with
a `#` get treated as comment.

##### MASKS (paragraph courtesy of `man incrontab.5`)
A file/folder can be watched for following events (specify them **comma**
separated only; **no** spaces or tabs )
- `* IN_ACCESS`; File was accessed (read) 
- `* IN_ATTRIB`; Metadata changed (permissions, timestamps, attributes, etc..)
- `* IN_CLOSE_WRITE`; File opened for writing was closed 
- `* IN_CLOSE_NOWRITE`; File not opened for writing was closed 
- `* IN_CREATE`; File/directory created in watched directory 
- `* IN_DELETE`; File/directory deleted from watched directory 
- `IN_DELETE_SELF`; Watched file/directory was itself deleted
- `* IN_MODIFY`; File was modified 
- `IN_MOVE_SELF`; Watched file/directory was itself moved
- `* IN_MOVED_FROM`; File moved out of watched directory 
- `* IN_MOVED_TO`; File moved into watched directory 
- `* IN_OPEN`; File was opened 

events marked with an asterisk trigger, when watching a folder, for files in
the watched category.

##### ATTRS
Specify them **together** with the masks, also *comma* separated only
- `recursive=true`; whether to recursively add watches in subdirectory or keep
  only the root one

##### ARGS
You can use following placeholders to pass information regarding the event to
the supplied command:
- `$$` -> single `$`
- `$@` -> path being watched
- `$#` -> filename that triggered the event; '' if event is triggered by
  watched folder
- `$%` -> triggered event masks as text
- `$&` -> triggered event masks as bits

#### list
Lists only lines parsed without errors. 

#### remove
Deletes user's `rsincron.table` (per default
`$HOME/.local/share/rsincron.table`).


## Configuration
Both `rsincrond` and `rsincrontab` look for a configuration file located under
`$HOME/.config/rsincron.toml`.
```toml
# Missing values from a config file default to the following
watch_table = "$HOME/.local/share/rsincron.table"
```

## Roadmap
- [ ] `rsincrontab`: `incrontab`'s sibling
    - [ ] add flags for
        - [x] *recursion* 
        - [ ] *dotdirs*
    - [ ] add more verbose output

- [ ] write every single type of test
- [ ] cleanup and reorganize code to allow more modularity
- [ ] write documentation


## About
This is a **very not ready** piece of software. Be ready for things not working
as expected.

I don't have an ETA yet since `rsincron` will be worked on during my spare time.
Feel free to message me for suggestions, critiques, hints or
contribution questions.

Also, I neved had a public repository. If you want to share some experience
on how to maintain one feel welcome to do so.

Please expect lots of bugs, `rsincron` isn't alpha yet. It looks closer to a
proof-of-concept at the moment.

## Known issues
- [ ] if started and watched folder isn't available daemon skips watch
- [ ] `mkdir -p watched_dir/{1,2,3}/2/3` doesn't trigger on subfolders
