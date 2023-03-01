# rsincron
![crates.io](https://img.shields.io/crates/v/rsincron.svg)

## Description
`rsincron` aims to be a drop-in replacement of the, it seems, abandoned
`incron` projected. 

You'll get two executables:
1. `rsincrontab`: **use** this to manage your table
2. `rsincrond`: the daemon itself. It isn't a daemon at the moment and I don't
   think I'll turn it into one. Use your favourite init system to manage it.

## Installation
### Cargo
Run ```cargo install rsincron```.

## Roadmap
- [ ] `rsincrontab`: `incrontab`'s sibling
	- [ ] add flags for *recursion* and *dotdirs*
	- [ ] add more verbose output

- [ ] `rsincrond`: the daemon itself
	- [ ] instantiate logging (somewhere has to be written which watches are
	  working and which aren't)
	- [ ] build some sort of *same flag* watch if a directory is made inside a 
	  watched one (with recursion **on**)
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
- [ ] daemon ignores events if watched folder is deleted and recreated while
  running
- [ ] no recursion is available at the moment
- [ ] if started and watched folder isn't available daemon skips watch
