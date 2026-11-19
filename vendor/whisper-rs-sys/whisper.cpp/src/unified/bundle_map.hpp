#pragma once
// A read-only memory map of a whole file, for the ced and voice components of a
// bundle (docs/bundle.md). ced.cpp and voice-detect.cpp load a bundle component
// from memory: they take the bytes of the whole bundle and a prefix, parse the
// header, and copy only the tensors of that component. Mapping the file means the
// operating system reads only the pages that are touched (the header and the
// component's tensors), so the other components are never read, and the map
// itself needs no private memory.
//
// The same code runs on Linux and macOS (mmap) and on Windows (MapViewOfFile).
// No temporary file is made. The file is opened read-only, mapped, and closed
// again before open() returns; the map stays valid until the object is destroyed.
//
// Keep the object alive only for the duration of the load. If another process
// shortens the file while it is mapped, a read past the new end can end the
// process (SIGBUS, or an access violation on Windows); model files must stay
// unchanged while they are loaded.
#include <cstddef>
#include <memory>
#include <string>

namespace pk {

class MappedFile {
public:
    // Returns nullptr and writes the reason to *err when the file cannot be
    // opened, is empty, or is too large to map in this process.
    static std::unique_ptr<MappedFile> open(const std::string& path, std::string* err);
    ~MappedFile();
    MappedFile(const MappedFile&) = delete;
    MappedFile& operator=(const MappedFile&) = delete;

    const void* data() const { return data_; }
    size_t size() const { return size_; }

private:
    MappedFile() = default;
    const void* data_ = nullptr;
    size_t size_ = 0;
#if defined(_WIN32)
    void* mapping_ = nullptr;   // HANDLE of the file mapping
#endif
};

}  // namespace pk
