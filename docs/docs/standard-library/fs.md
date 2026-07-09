---
icon: lucide/file-digit
---
# File system library

This library is included by default, you don't need to import it.

## Read

`fs::read(p: string) -> string`<br/>
Returns the contents of the file with path `p`.
```
print(fs::read("hello_world.txt"));
```

## Exists

`fs::exists(p: string) -> bool`<br/>
Returns whether or not the file `p` exists.
```
print(fs::exists("exists.txt")); // prints true
print(fs::exists("does_not_exist.txt")) // prints false
```

## Write

`fs::write(path: string, contents: string)`<br/>
Writes `contents` to the file located at `path`, overwriting any existing content. Creates `path` if it doesn't exist.
```
fs::write("test.txt", "Hello, World!");
```

## Append

`fs::append(path: string, contents: string)`<br/>
Appends `contents` to the file located at `path`. Creates `path` if it doesn't exist.
```
fs::append("test.txt", "Hello, World!");
```

## Delete

`fs::delete(path: string)`<br/>
Deletes the file located at `path`. Crashes if the file doesn't exist.
```
fs::delete("bad_file.txt");
```

## DeleteDir

`fs::delete_dir(path: string)`<br/>
Deletes the empty folder located at `path`. Crashes if the folder doesn't exist or isn't empty.
```
fs::delete_dir("bad_folder/");
```