# Dynamic Syscalls

## Purpose

The purpose of this project is to reduce the friction involved in making a system call directly from userspace. It allows you to invoke syscalls without the need to write or compile a full C or Rust program — just provide the syscall number and up to six arguments, and it will do the rest.

## Usage

The first argument is the **syscall number**. You do **not** need to type-hint it.
The next arguments (up to a maximum of six) **must** be type-hinted:

* Use `n:<value>` to indicate an unsigned number (`usize`)
* Use `s:<value>` to indicate a string whose pointer should be passed

Arguments are passed in the order expected by the syscall.

## Examples

If you want to use the `write(2)` syscall — instead of bothering with `echo(1)` or `printf(1)` — you can use this program to directly interface with the kernel.

Reference for syscall numbers: [Linux System Call Table (x86\_64)](https://blog.rchapman.org/posts/Linux_System_Call_Table_for_x86_64/)

### Writing to stdout

System call number 1 corresponds to `write`, which takes:

1. A file descriptor (e.g., `1` for `stdout`)
2. A pointer to the buffer (e.g., `"Hello, World\n"`)
3. The number of bytes to write (e.g., `13`, including the newline)

Example:

```bash
./dynamic-syscall 1 n:1 $'s:Hello, World\n' n:13
```

**Note:** To correctly pass special characters like `\n`, use **ANSI C quoting** by prefixing your string with `$` and enclosing it in single quotes. Otherwise, characters like `\n` will be passed as literal backslashes and `n` (i.e., `"\\n"`), due to how BASH handles escaping.

### Sending SIGKILL to a process

1. Identify the PID of the target process (e.g., `414195`).
2. Run:

```bash
./dynamic-syscall 62 n:414195 n:9
```

Explanation:

* Syscall 62 is `kill(2)`, which takes:

  * A `pid` (`pid_t`) — the ID of the process to signal
  * A signal number (`int`) — in this case, `9` (`SIGKILL`) to immediately terminate the process

## Limitations

Currently, only two argument types are supported:

* `s:` — string (converted to a pointer)
* `n:` — unsigned number (`usize`)

You **can** pass negative numbers, and they usually work, because casting from `i32` to `usize` preserves the bit pattern. However, this is architecture-dependent and not type-safe — be cautious.

## Future Improvements (TODO)

* Allow plain numbers (e.g., `42`) to default to `usize` without requiring the `n:` prefix?
* Add support for additional types (`i32`, `NULL`, raw pointers, flags, etc.)
* Optional unescaping of common character sequences (`\n`, `\t`, etc.) internally

