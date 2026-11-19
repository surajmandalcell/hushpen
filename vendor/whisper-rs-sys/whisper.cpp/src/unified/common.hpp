#pragma once
#include <cstdio>
#include <string>
#define PK_LOG(...)  do { std::fprintf(stderr, "[parakeet] " __VA_ARGS__); std::fprintf(stderr, "\n"); } while (0)

namespace pk {

// Writes `bytes` to `path` so a reader sees either the old file or the whole
// new one. It writes `<path>.tmp` in the same directory, checks every write
// and the close, then replaces `path` with it (MoveFileExA on Windows, rename
// elsewhere). On any failure it removes the tmp file, leaves `path` as it was,
// sets `*err` (when not null) to a one-line reason and returns false.
bool write_file_atomic(const std::string& path, const std::string& bytes, std::string* err);

}  // namespace pk
