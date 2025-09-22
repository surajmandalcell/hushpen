# ASR Pro Performance Report

**Audio:** 158.4s (2.6min) | **Generated:** 02:13:48 | **Duration:** 30.3s

## Results

| Model | Backend | Load | Transcribe | RTF | WPS | Status |
|-------|---------|------|------------|-----|-----|--------|
| whisper-tiny_detected | ERROR | - | - | - | - | FAIL |
| whisper-tiny_cpu | ERROR | - | - | - | - | FAIL |
| whisper-tiny_cuda | ERROR | - | - | - | - | FAIL |
| whisper-base | detected->cuda | 1.1s | 1.4s | 115.1x | 244 | PASS |
| whisper-base | cpu->cpu | 0.7s | 1.6s | 96.7x | 205 | PASS |
| whisper-base | cuda->cuda | 0.7s | 0.9s | 170.7x | 362 | PASS |
| parakeet-tdt-0.6b-v2 | detected->cuda | 2.5s | 1.3s | 119.3x | 1380 | PASS |
| parakeet-tdt-0.6b-v2 | cpu->cpu | 3.6s | 10.1s | 15.7x | 182 | PASS |
| parakeet-tdt-0.6b-v2 | cuda->cuda | 2.4s | 1.3s | 117.6x | 1361 | PASS |

## Backend Comparison

**CUDA:** 1.7s load, 1.2s transcribe, 130.7x RTF
**CPU:** 2.2s load, 5.9s transcribe, 56.2x RTF

## Best
**Fastest:** whisper-base_cuda (0.9s)
**Most Efficient:** whisper-base_cuda (170.7x RTF)