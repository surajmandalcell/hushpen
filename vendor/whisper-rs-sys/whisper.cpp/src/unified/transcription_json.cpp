#include "transcription_json.hpp"
#include "buffered_streaming.hpp"

#include <cstdio>

namespace pk {

// Adapted from the upstream C API's JSON string writer.
static void append_json_string(std::string& out, const std::string& s) {
    out += '"';
    char esc[8];
    for (unsigned char c : s) {
        switch (c) {
            case '"':  out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            default:
                if (c < 0x20) {
                    std::snprintf(esc, sizeof(esc), "\\u%04x", (unsigned)c);
                    out += esc;
                } else {
                    out += (char)c;
                }
        }
    }
    out += '"';
}

std::string transcription_to_json(const std::string& text, const std::vector<Word>& words) {
    std::string out = "{\"text\":";
    append_json_string(out, text);
    out += ",\"words\":[";
    for (size_t i = 0; i < words.size(); ++i) {
        if (i) out += ',';
        out += "{\"word\":";
        append_json_string(out, words[i].text);
        out += ",\"start\":" + format_fixed(words[i].start, 3);
        out += ",\"end\":" + format_fixed(words[i].end, 3) + "}";
    }
    return out + "]}";
}
} // namespace pk
