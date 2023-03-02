# rsincron
![crates.io](https://img.shields.io/crates/v/rsincron.svg)

## Description
`rsincron` aims to be a drop-in replacement of the, it seems, abandoned
`incron` projected. 

You'll get two executables:
1. `rsincrontab`: **use** this to manage your table
2. `rsincrond`: the daemon itself. It isn't a daemon at the moment and I don't
   think I'll turn it into one. Use your favourite init system to manage it.

### Usage
**Please use** `rsincrontab -e` to edit your *rsincron*'s table. It discards
invalid lines, while giving error feedback and keeps it formatted correctly.

#### `rsincrontab` format
It has the same format readable under `EXAMPLE` section in `man incrontab.5`.
```
<path> <event_masks> <command to run>
```

Keep following in mind:
- if the path doesn't exist before running `rsincrond` the line gets skipped
  (it's on the *TODO* list)
- you can specify how many event masks you want separated by a single comma (`,`). **Do not use whitespace**.
- flags (such as the only one implemented `recursive=true`) have to be added,
  comma separated, together with the masks
- the rest gets parse as the `COMMAND` to be run as `bash -c "$COMMAND"`

#### `rsincrontab` expansion
Following character combinations get expanded before being run:
- `$$` -> single `$`
- `$@` -> path being watched
- `$#` -> filename that triggered the event; '' if event is triggered by
  watched folder
- `$%` -> triggered event masks as text
- `$&` -> triggered event masks as bits


## Installation
### Cargo
Run ```cargo install rsincron```.

## Roadmap
- [ ] `rsincrontab`: `incrontab`'s sibling
	- [ ] add flags for
		- [x] *recursion* 
		- [ ] *dotdirs*
	- [ ] add more verbose output

- [ ] `rsincrond`: the daemon itself
	- [ ] instantiate logging (somewhere has to be written which watches are
	  working and which aren't)
	- [ ] build some sort of *same flag* watch if a directory is made inside a 
	  watched one (with recursion **on**)

- [ ] write every single type of test
- [ ] cleanup and reorganize code to allow more modularity
- [ ] write documentation

### Currently working on
Some sort of runtime checks:
- [ ] loop (with configurable) polling time checks for missed
  folders, if recursion is on, and adds them to the active watches
- [ ] general, configurable, logging (now it's very minimal to stdin/stderr)

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
- [ ] daemon ignores events if watched folder is deleted and recreated while
  running
- [x] no recursion is available at the moment
- [ ] if started and watched folder isn't available daemon skips watch
