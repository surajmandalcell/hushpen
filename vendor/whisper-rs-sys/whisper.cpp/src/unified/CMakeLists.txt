# ASR sources from mudler/parakeet.cpp at 2eac2e77d9c7ef8899f7a86a46e693ac5ac1a5cd (MIT).
target_sources(parakeet PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/hushpen.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/backend.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/buffered_streaming.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/bundle.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/bundle_map.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/conformer.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/encoder.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/exact_matvec.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/fft.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/joint.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/mel.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/model_loader.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/pos_enc.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/prediction.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/prompt_kernel.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/relpos_attention.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/rnnt.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/subsampling.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/ternary.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/ternary_kernels_neon.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/ternary_kernels_x86.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/tokenizer.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/transcription.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/transcription_json.cpp)
target_compile_features(parakeet PRIVATE cxx_std_17)

add_executable(hushpen-unified-pacing-check EXCLUDE_FROM_ALL tests/pacing.cpp)
target_compile_features(hushpen-unified-pacing-check PRIVATE cxx_std_17)
target_link_libraries(hushpen-unified-pacing-check PRIVATE parakeet)
