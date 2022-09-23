# mntime command

This `mntime` command internally uses [time -l][time] or [gtime -v][gtime](GNU Time) to calculate the mean.

[time]:https://www.freebsd.org/cgi/man.cgi?query=time
[gtime]:https://man7.org/linux/man-pages/man1/time.1.html

The `time -l`[time] option and `gtime -v`[gtime] option measure memory usage as well as execution time.
If you are only measuring execution time, you can use shell built-in `time` instead.

The project name comes from **m** commands and **n** times and also from **m**ea**n**. Besides, it is taken from **m**ultiple **n**umber **time**.

So, `mntime` executes the specified m commands n times and calculates the mean.

## Demo

TODO

## Installation

TODO

## Usage

Please use the `-h`/`--help` option for more information.

### Basic benchmarks

```sh
mntime sleep 1
```

TODO: result

If the number of runs is not specified, it will run 5 times. If you want to change the number of runs, you can use the `-r`/`--runs` option.

```sh
mntime --runs 10 sleep 1
```

TODO: result

### Compare benchmarks

```sh
mntime 'sleep 1' 'sleep 0.9' 'sleep 1.1'
```

OR

```sh
mntime sleep 1 -- sleep 0.9 -- sleep 1.1
```

TODO: result

When multiple commands are specified in this way, each is executed n times, the mean is calculated, and comparisons are made.

If only a command with no arguments is specified, the 'command' enclosure cannot be specified. "--" delimiters should be used. It is possible to mix both. For example,

```sh
# All the same
mntime 'command1 --flag arg' command2 -- 'command3 -f -- args'
mntime 'command1' --flag arg -- command2 -- 'command3 -f -- args'
## Following is recommended
mntime command1 --flag arg -- command2 -- 'command3 -f -- args'
```

"--" delimiters are recommended, except when they contain " -- ".

## Alternative tools

`mntime` is inspired by [hyperfine](https://github.com/sharkdp/hyperfine).

## Note

English documentation is written while using DeepL.

## License

"mntime" is under [zlib License](./LICENSE). Please feel free to use this, but no warranty.

Also, the license of [gnu-time][gtime] is GNU GENERAL PUBLIC LICENSE Version 3.
