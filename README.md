# dodx

*a differential operator for CLI tools.*

dodx creates patch files by comparing command outputs with original files.

![Example](images/example.gif)

## Examples

### Basic usage

``` console
$ cat 1.txt
foo
$ echo 1.txt | dodx sed 's/foo/bar/g'
--- 1.txt
+++ 1.txt
@@ -1 +1 @@
-foo
+bar
$ echo 1.txt | dodx sed 's/foo/bar/g' | patch -p0
patching file 1.txt
$ cat 1.txt
bar
```

### With a single argument

``` sh
dodx -x sed 's/foo/bar/g' 1.txt
```

### With multiple arguments

``` sh
dodx -X sed 's/foo/bar/g' -- *.txt
```

### As a filter

``` sh
echo foo | dodx -F sed 's/foo/bar/g'
```

Output:

``` diff
--- <stdin>
+++ <stdout>
@@ -1 +1 @@
-foo
+bar
```

### With `find`

``` sh
find . -name '*.txt' | dodx sed 's/foo/bar/g'
# or
find . -name '*.txt' -print0 | dodx -0 sed 's/foo/bar/g'
# or
find . -name '*.txt' -exec dodx -x sed 's/foo/bar/g' '{}' ';'
# or
find . -name '*.txt' -exec dodx -X sed 's/foo/bar/g' -- '{}' +
```

### With `fd`

``` sh
fd '\.txt$' | dodx sed 's/foo/bar/g'
# or
fd -0 '\.txt$' | dodx -0 sed 's/foo/bar/g'
# or
fd '\.txt$' -x dodx -x sed 's/foo/bar/g'
# or
fd '\.txt$' -X dodx -X sed 's/foo/bar/g' --
```

### With `rg`

``` sh
rgdiff() {
    pat="$1"
    rep="$2"
    shift 2
    rg -0l "$pat" "$@" | dodx -0u rg "$pat" -r "$rep" -IN --passthru
}

rgdiff foo bar -g '*.txt'
```

## Usage

``` console
$ dodx --help
dodx creates patch files by comparing command outputs with original files

Usage: dodx [OPTIONS] <CMD> [ARG]...

Arguments:
  <CMD>     Command to execute
  [ARG]...  Command arguments

Options:
  -0, --null                     Handle null-separated input items
  -j, --threads <THREADS>        The approximate number of threads to use [default: 0]
  -u, --unordered                Produce fast unordered output in multi-threaded execution
  -X, --multi-args               Interpret arguments after last '--' as file names
  -x, --single-arg               Interpret the last argument as a file name
  -f, --files-from <FILES_FROM>  File containing file names
  -F, --filter                   Show diff between CMD's stdin and stdout
  -h, --help                     Print help
  -V, --version                  Print version
```

## License

MIT or Apache-2.0
