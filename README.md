# mntime command

This `mntime` command internally uses [gnu-time][gtime] to calculate the mean.

[gtime]:https://www.gnu.org/software/time/

[gnu-time][gtime] is like an extension of the `time` command, allowing you to measure memory usage as well as execution time.

The project name comes from **m** commands and **n** times and also from **m**ea**n**.

So, `mntime` executes the specified m commands n times and calculates the mean.

## Demo

TODO
`gnu-time` is required.

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

## Alternative tools

`mntime` is inspired by [hyperfine](https://github.com/sharkdp/hyperfine).

## Note

English documentation is written while using DeepL.

## License

"mntime" is under [zlib License](./LICENSE). Please feel free to use this, but no warranty.

Also, the license of [gnu-time][gtime] is GNU GENERAL PUBLIC LICENSE Version 3.
