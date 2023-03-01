# rsincron
## Description
`rsincron` aims to be a drop-in replacement of the, it seems, abandoned
`incron` projected. 

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

## Known issues
- [ ] daemon ignores events if watched folder is deleted and recreated while
  running
- [ ] no recursion is available at the moment
- [ ] if started and watched folder isn't available daemon skips watch
