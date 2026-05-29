# File System library

> T represents any type.\
> Array\<T\> represents an array containing elements of type T\
> \<T\> represents any element of type T

## Read

`fs::read(p: String) -> String`\
Returns the contents of the file with path `p`.
Example:
```
print(fs::read("hello_world.txt"));
```

## Exists

`fs::exists(p: String) -> Bool`\
Returns whether or not the file `p` exists.
Example:
```
print(fs::exists("exists.txt")); // returns true
print(fs::exists("does_not_exist.txt")) // returns false
```

## Write

`fs::write(path: String, contents: String)`\
Writes `contents` to `path`, overwriting any existing content. Creates `path` if it doesn't exist.
Example:
```
fs::write("test.txt", "Hello, World!");
```

## Append

`fs::append(path: String, contents: String)`\
Appends `contents` to `path`. Creates `path` if it doesn't exist.
Example:
```
fs::append("test.txt", "Hello, World!");
```

## Delete

`fs::delete(path: String)`\
Deletes the file located at `path`. Crashes if the file doesn't exist.
Example:
```
fs::delete("bad_file.txt");
```

## Delete_dir

`fs::delete_dir(path: String)`\
Deletes the empty folder located at `path`. Crashes if the folder doesn't exist or isn't empty.
Example:
```
fs::delete_dir("bad_folder/");
```