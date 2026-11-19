#pragma once
#include "transcription.hpp"

namespace pk {
std::string transcription_to_json(const std::string& text, const std::vector<Word>& words);
}
