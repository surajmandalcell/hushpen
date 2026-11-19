#include "bundle_map.hpp"

#include <cstdint>
#include <limits>

#if defined(_WIN32)
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#else
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#endif

namespace pk {

namespace {
void set_err(std::string* err, const std::string& m) {
    if (err) *err = m;
}
}  // namespace

#if defined(_WIN32)

MappedFile::~MappedFile() {
    if (data_) UnmapViewOfFile(data_);
    if (mapping_) CloseHandle(static_cast<HANDLE>(mapping_));
}

std::unique_ptr<MappedFile> MappedFile::open(const std::string& path, std::string* err) {
    HANDLE f = CreateFileA(path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING,
                           FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE) { set_err(err, "cannot open " + path); return nullptr; }
    LARGE_INTEGER sz;
    if (!GetFileSizeEx(f, &sz) || sz.QuadPart <= 0 || (uint64_t)sz.QuadPart > std::numeric_limits<size_t>::max()) {
        CloseHandle(f);
        set_err(err, "cannot map " + path + " (empty or too large)");
        return nullptr;
    }
    std::unique_ptr<MappedFile> m(new MappedFile());
    HANDLE h = CreateFileMappingA(f, nullptr, PAGE_READONLY, 0, 0, nullptr);
    CloseHandle(f);   // the mapping keeps the file open
    if (!h) { set_err(err, "cannot map " + path); return nullptr; }
    m->mapping_ = h;
    m->data_ = MapViewOfFile(h, FILE_MAP_READ, 0, 0, 0);
    if (!m->data_) { set_err(err, "cannot map " + path); return nullptr; }
    m->size_ = (size_t)sz.QuadPart;
    return m;
}

#else

MappedFile::~MappedFile() {
    if (data_) ::munmap(const_cast<void*>(data_), size_);
}

std::unique_ptr<MappedFile> MappedFile::open(const std::string& path, std::string* err) {
    const int fd = ::open(path.c_str(), O_RDONLY | O_CLOEXEC);
    if (fd < 0) { set_err(err, "cannot open " + path); return nullptr; }
    struct stat st {};
    if (::fstat(fd, &st) != 0 || st.st_size <= 0 || (uint64_t)st.st_size > std::numeric_limits<size_t>::max()) {
        ::close(fd);
        set_err(err, "cannot map " + path + " (empty or too large)");
        return nullptr;
    }
    void* p = ::mmap(nullptr, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0);
    ::close(fd);   // the mapping stays valid
    if (p == MAP_FAILED) { set_err(err, "cannot map " + path + " into memory"); return nullptr; }
    std::unique_ptr<MappedFile> m(new MappedFile());
    m->data_ = p;
    m->size_ = (size_t)st.st_size;
    return m;
}

#endif

}  // namespace pk
