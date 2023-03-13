# dodx

a differential operator for CLI tools.

## Usage

### Basic usage

```sh-session
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

```sh
dodx -x sed 's/foo/bar/g' 1.txt
```

### With multiple arguments

```sh
dodx -X sed 's/foo/bar/g' -- *.txt
```

### As a filter

```sh
dodx -F sed 's/foo/bar/g' <1.txt
```

### With `find`

```sh
find . -name '*.txt' | dodx sed 's/foo/bar/g'
# or
find . -name '*.txt' -print0 | dodx -0 sed 's/foo/bar/g'
# or
find . -name '*.txt' -exec dodx -x sed 's/foo/bar/g' '{}' ';'
# or
find . -name '*.txt' -exec dodx -X sed 's/foo/bar/g' -- '{}' +
```

### With `fd`

```sh
fd '\.txt$' | dodx sed 's/foo/bar/g'
# or
fd -0 '\.txt$' | dodx -0 sed 's/foo/bar/g'
# or
fd '\.txt$' -x dodx -x sed 's/foo/bar/g'
# or
fd '\.txt$' -X dodx -X sed 's/foo/bar/g' --
```

### With `rg`

```sh
rgdiff() {
    pat="$1"
    rep="$2"
    shift 2
    rg -0l "$pat" "$@" | dodx -0 rg "$pat" -r "$rep" -IN --passthru
}

rgdiff foo bar -g '*.txt'
```

## License

MIT or Apache-2.0
