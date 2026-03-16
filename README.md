# ELF Symbol Size Differ

`elf-diff` is a command-line tool that calculates and reports the size differences of symbols between two ELF files. It's particularly useful for tracking binary size impact of code changes in embedded projects.

## Features

- Compares two ELF files and identifies added, removed, or changed symbols.
- Calculates the size difference for each symbol.
- Supports demangling of C++ symbol names.
- Provides output in either a human-readable table or a machine-readable CSV format.

## Usage

```bash
elf-diff [OPTIONS] <FILE1> <FILE2>
```

### Arguments

- `<FILE1>`: The first ELF file to compare.
- `<FILE2>`: The second ELF file to compare.

### Options

- `-o, --output <OUTPUT>`: Sets the output format.  
  - `table` (default): Prints a formatted table to the console.  
  - `csv`: Prints the output in CSV format.
- `-d, --demangle`: Demangles the symbol names before comparison.
- `-h, --help`: Prints help information.
- `-V, --version`: Prints version information.

## Example

Assuming you have two versions of an ELF file, `old.elf` and `new.elf`:

```bash
elf-diff --demangle new.elf old.elf
```

This will produce a report showing the symbol size differences between the two files, with demangled names. The output might look something like this:

**Table Output:**

```
| Symbol Name          | Change  | Size Difference |
|----------------------|---------|-----------------|
| new_function()       | ADDED   | +128            |
| old_function()       | REMOVED | -64             |
| modified_function()  | CHANGED | +16             |
```

**CSV Output:**

```csv
name,change_type,size_diff
new_function(),ADDED,128
old_function(),REMOVED,-64
modified_function(),CHANGED,16
```
