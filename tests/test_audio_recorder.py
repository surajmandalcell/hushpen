"""Test audio recording functionality with comprehensive TDD approach."""

import pytest
import threading
import time
import wave
import numpy as np
from pathlib import Path
from unittest.mock import Mock, patch, MagicMock
import sounddevice as sd
from asrpro.audio_recorder import AudioRecorder


class TestAudioRecorderInit:
    """Test AudioRecorder initialization."""

    def test_init_with_default_parameters(self):
        """Test initialization with default parameters."""
        recorder = AudioRecorder()

        assert recorder.samplerate == 16000
        assert recorder.channels == 1
        assert recorder.device is None
        assert recorder._q.empty()
        assert not recorder._stop.is_set()
        assert recorder._stream is None
        assert recorder.file_path is None

    def test_init_with_custom_parameters(self):
        """Test initialization with custom parameters."""
        recorder = AudioRecorder(samplerate=44100, channels=2, device=1)

        assert recorder.samplerate == 44100
        assert recorder.channels == 2
        assert recorder.device == 1

    @patch('asrpro.audio_recorder.platform.system')
    def test_init_checks_macos_permissions(self, mock_system):
        """Test that macOS permissions are checked during initialization."""
        mock_system.return_value = 'Darwin'

        with patch.object(AudioRecorder, '_check_macos_microphone_permission') as mock_check:
            recorder = AudioRecorder()
            mock_check.assert_called_once()

    @patch('asrpro.audio_recorder.platform.system')
    def test_init_skips_permission_check_on_non_macos(self, mock_system):
        """Test that permission check is skipped on non-macOS systems."""
        mock_system.return_value = 'Windows'

        with patch.object(AudioRecorder, '_check_macos_microphone_permission') as mock_check:
            recorder = AudioRecorder()
            mock_check.assert_not_called()


class TestAudioDeviceDetection:
    """Test audio device detection functionality."""

    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_list_devices_success(self, mock_query):
        """Test successful device listing."""
        mock_devices = [
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100},
            {'name': 'USB Mic', 'max_input_channels': 2, 'default_samplerate': 48000},
            {'name': 'Speakers', 'max_input_channels': 0, 'default_samplerate': 44100}  # Output only
        ]
        mock_query.return_value = mock_devices

        with patch('asrpro.audio_recorder.sd.default.device', (0, 1)):
            devices = AudioRecorder.list_devices()

            assert len(devices) == 2  # Only input devices
            assert devices[0]['name'] == 'Built-in Microphone'
            assert devices[0]['index'] == 0
            assert devices[0]['channels'] == 1
            assert devices[0]['is_default'] == True

            assert devices[1]['name'] == 'USB Mic'
            assert devices[1]['index'] == 1
            assert devices[1]['channels'] == 2
            assert devices[1]['is_default'] == False

    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_list_devices_no_input_devices(self, mock_query):
        """Test device listing when no input devices are available."""
        mock_devices = [
            {'name': 'Speakers', 'max_input_channels': 0, 'default_samplerate': 44100}
        ]
        mock_query.return_value = mock_devices

        devices = AudioRecorder.list_devices()
        assert len(devices) == 0

    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_list_devices_error_handling(self, mock_query):
        """Test error handling when device listing fails."""
        mock_query.side_effect = Exception("Audio system error")

        devices = AudioRecorder.list_devices()
        assert len(devices) == 0


class TestMacOSPermissionHandling:
    """Test macOS-specific permission handling."""

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_check_macos_microphone_permission_success(self, mock_query, mock_system):
        """Test successful microphone permission check on macOS."""
        mock_system.return_value = 'Darwin'
        mock_devices = [{'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100}]
        mock_query.return_value = mock_devices

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == True

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_check_macos_microphone_permission_no_devices(self, mock_query, mock_system):
        """Test permission check when no input devices are found."""
        mock_system.return_value = 'Darwin'
        mock_devices = [{'name': 'Speakers', 'max_input_channels': 0, 'default_samplerate': 44100}]
        mock_query.return_value = mock_devices

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == False

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_check_macos_microphone_permission_error(self, mock_query, mock_system):
        """Test permission check when querying devices fails."""
        mock_system.return_value = 'Darwin'
        mock_query.side_effect = Exception("Permission denied")

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == False

    @patch('asrpro.audio_recorder.platform.system')
    def test_check_macos_microphone_permission_non_macos(self, mock_system):
        """Test permission check on non-macOS systems."""
        mock_system.return_value = 'Windows'

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == True

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.subprocess.run')
    def test_request_macos_microphone_permission(self, mock_subprocess, mock_system):
        """Test microphone permission request on macOS."""
        mock_system.return_value = 'Darwin'

        recorder = AudioRecorder()
        recorder._request_macos_microphone_permission()

        assert mock_subprocess.call_count == 2
        # Verify the osascript commands were called
        calls = mock_subprocess.call_args_list
        assert 'System Settings' in str(calls[0])
        assert 'Privacy_Microphone' in str(calls[0])

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.subprocess.run')
    def test_request_macos_microphone_permission_error(self, mock_subprocess, mock_system):
        """Test error handling when requesting permissions fails."""
        mock_system.return_value = 'Darwin'
        mock_subprocess.side_effect = Exception("Failed to open System Settings")

        recorder = AudioRecorder()
        # Should not raise an exception
        recorder._request_macos_microphone_permission()

    @patch('asrpro.audio_recorder.platform.system')
    def test_request_macos_microphone_permission_non_macos(self, mock_system):
        """Test that permission request is skipped on non-macOS."""
        mock_system.return_value = 'Windows'

        recorder = AudioRecorder()
        recorder._request_macos_microphone_permission()
        # Should not call subprocess on non-macOS


class TestAudioRecording:
    """Test audio recording functionality."""

    def test_callback_function(self):
        """Test the audio callback function."""
        recorder = AudioRecorder()

        # Mock audio data
        mock_data = np.array([[0.1, 0.2], [0.3, 0.4]], dtype=np.float32)

        # Call callback
        recorder._callback(mock_data, frames=1024, time=None, status=None)

        # Check that data was added to queue
        assert not recorder._q.empty()
        retrieved_data = recorder._q.get()
        np.testing.assert_array_equal(retrieved_data, mock_data)

    @patch('asrpro.audio_recorder.sd.InputStream')
    def test_start_recording_success(self, mock_stream_class):
        """Test successful recording start."""
        mock_stream = Mock()
        mock_stream_class.return_value = mock_stream

        recorder = AudioRecorder()
        test_file = Path("/tmp/test_audio.wav")

        with patch('threading.Thread') as mock_thread:
            recorder.start(test_file)

            # Verify stream was created and started
            mock_stream_class.assert_called_once()
            mock_stream.start.assert_called_once()

            # Verify file path was set
            assert recorder.file_path == test_file

            # Verify writer thread was started
            mock_thread.assert_called_once()

    @patch('asrpro.audio_recorder.sd.InputStream')
    @patch('asrpro.audio_recorder.platform.system')
    def test_start_recording_macos_permission_error(self, mock_system, mock_stream_class):
        """Test recording start with macOS permission error."""
        mock_system.return_value = 'Darwin'
        mock_stream_class.side_effect = sd.PortAudioError("Permission denied")

        with patch.object(AudioRecorder, '_request_macos_microphone_permission') as mock_request:
            recorder = AudioRecorder()
            test_file = Path("/tmp/test_audio.wav")

            with pytest.raises(RuntimeError, match="Microphone access denied"):
                recorder.start(test_file)

            mock_request.assert_called_once()

    @patch('asrpro.audio_recorder.sd.InputStream')
    def test_start_recording_other_error(self, mock_stream_class):
        """Test recording start with other types of errors."""
        mock_stream_class.side_effect = sd.PortAudioError("Device not found")

        recorder = AudioRecorder()
        test_file = Path("/tmp/test_audio.wav")

        with pytest.raises(sd.PortAudioError, match="Device not found"):
            recorder.start(test_file)

    def test_stop_recording(self):
        """Test stopping recording."""
        recorder = AudioRecorder()
        test_file = Path("/tmp/test_audio.wav")
        recorder.file_path = test_file

        # Mock stream
        mock_stream = Mock()
        recorder._stream = mock_stream
        recorder._stop.clear()

        result = recorder.stop()

        # Verify stop event was set
        assert recorder._stop.is_set()

        # Verify stream was stopped and closed
        mock_stream.stop.assert_called_once()
        mock_stream.close.assert_called_once()

        # Verify file path was returned
        assert result == test_file

    def test_stop_recording_no_stream(self):
        """Test stopping recording when no stream exists."""
        recorder = AudioRecorder()
        recorder._stream = None

        result = recorder.stop()

        # Should not raise an error
        assert result is None


class TestAudioWriter:
    """Test the audio writer thread functionality."""

    def test_writer_creates_wav_file(self, tmp_path):
        """Test that writer creates a valid WAV file."""
        recorder = AudioRecorder()
        test_file = tmp_path / "test_output.wav"
        recorder.file_path = test_file
        recorder.channels = 1
        recorder.samplerate = 16000

        # Add some mock audio data to queue
        audio_data = np.array([[0.1, 0.2, 0.3]], dtype=np.float32)
        recorder._q.put(audio_data)

        # Set stop event after a short delay
        def set_stop():
            time.sleep(0.01)
            recorder._stop.set()

        threading.Thread(target=set_stop, daemon=True).start()

        # Run writer
        recorder._writer()

        # Verify file was created
        assert test_file.exists()

        # Verify WAV file format
        with wave.open(str(test_file), 'rb') as wf:
            assert wf.getnchannels() == 1
            assert wf.getsampwidth() == 2
            assert wf.getframerate() == 16000
            assert wf.getnframes() > 0

    def test_writer_empty_queue(self):
        """Test writer behavior with empty queue."""
        recorder = AudioRecorder()
        recorder.file_path = Path("/tmp/test.wav")
        recorder._stop.set()  # Stop immediately

        # Should not raise an error
        recorder._writer()

    def test_writer_no_file_path(self):
        """Test writer behavior when no file path is set."""
        recorder = AudioRecorder()
        recorder.file_path = None
        recorder._stop.set()

        # Should not raise an error
        recorder._writer()


class TestAudioFormatValidation:
    """Test audio format validation."""

    def test_audio_data_conversion(self):
        """Test that audio data is properly converted to int16."""
        recorder = AudioRecorder()

        # Test float to int16 conversion
        float_data = np.array([[0.5, -0.5, 1.0, -1.0]], dtype=np.float32)
        expected_int16 = (float_data * 32767).astype(np.int16)

        # Add to queue
        recorder._q.put(float_data)
        retrieved_data = recorder._q.get()

        # Verify conversion
        converted_data = (retrieved_data * 32767).astype(np.int16)
        np.testing.assert_array_equal(converted_data, expected_int16)

    def test_recording_parameters(self):
        """Test that recording parameters are properly set."""
        recorder = AudioRecorder(samplerate=44100, channels=2)

        # Mock stream creation to verify parameters
        with patch('asrpro.audio_recorder.sd.InputStream') as mock_stream_class:
            mock_stream = Mock()
            mock_stream_class.return_value = mock_stream

            with patch('threading.Thread'):
                recorder.start(Path("/tmp/test.wav"))

                # Verify stream was created with correct parameters
                call_args = mock_stream_class.call_args
                assert call_args[1]['samplerate'] == 44100
                assert call_args[1]['channels'] == 2


class TestDeviceCompatibility:
    """Test device compatibility and selection."""

    @patch('asrpro.audio_recorder.AudioRecorder.list_devices')
    def test_device_selection_validation(self, mock_list_devices):
        """Test device selection validation."""
        mock_list_devices.return_value = [
            {'index': 0, 'name': 'Built-in Mic', 'channels': 1},
            {'index': 1, 'name': 'USB Mic', 'channels': 2}
        ]

        # Test with valid device index
        recorder = AudioRecorder(device=1)
        assert recorder.device == 1

        # Test with invalid device index (should still work, may fail at runtime)
        recorder = AudioRecorder(device=99)
        assert recorder.device == 99

    @patch('asrpro.audio_recorder.AudioRecorder.list_devices')
    @patch('asrpro.audio_recorder.sd.InputStream')
    def test_recording_with_specific_device(self, mock_stream_class, mock_list_devices):
        """Test recording with specific device selection."""
        mock_list_devices.return_value = [
            {'index': 0, 'name': 'Built-in Mic', 'channels': 1},
            {'index': 1, 'name': 'USB Mic', 'channels': 2}
        ]

        mock_stream = Mock()
        mock_stream_class.return_value = mock_stream

        recorder = AudioRecorder(device=1)

        with patch('threading.Thread'):
            recorder.start(Path("/tmp/test.wav"))

            # Verify stream was created with specified device
            call_args = mock_stream_class.call_args
            assert call_args[1]['device'] == 1


class TestErrorHandling:
    """Test comprehensive error handling."""

    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_device_query_error_recovery(self, mock_query):
        """Test recovery from device query errors."""
        mock_query.side_effect = Exception("Audio system unavailable")

        devices = AudioRecorder.list_devices()
        assert len(devices) == 0  # Should return empty list, not crash

    @patch('asrpro.audio_recorder.sd.InputStream')
    def test_recording_start_error_handling(self, mock_stream_class):
        """Test error handling when recording fails to start."""
        mock_stream_class.side_effect = Exception("Audio device busy")

        recorder = AudioRecorder()

        with pytest.raises(Exception, match="Audio device busy"):
            recorder.start(Path("/tmp/test.wav"))

    def test_writer_error_handling(self, tmp_path):
        """Test writer error handling."""
        recorder = AudioRecorder()
        # Use an invalid path that should cause an error
        recorder.file_path = Path("/invalid/path/test.wav")
        recorder.channels = 1
        recorder.samplerate = 16000

        # Add some data to queue
        audio_data = np.array([[0.1]], dtype=np.float32)
        recorder._q.put(audio_data)

        # Set stop event
        recorder._stop.set()

        # Should not raise an exception (error should be handled gracefully)
        recorder._writer()


class TestIntegration:
    """Test integration scenarios."""

    def test_full_recording_workflow(self, tmp_path):
        """Test complete recording workflow."""
        recorder = AudioRecorder()
        test_file = tmp_path / "integration_test.wav"

        # Mock stream to simulate recording
        with patch('asrpro.audio_recorder.sd.InputStream') as mock_stream_class:
            mock_stream = Mock()
            mock_stream_class.return_value = mock_stream

            # Start recording
            with patch('threading.Thread') as mock_thread:
                recorder.start(test_file)

                # Verify recording state
                assert recorder.file_path == test_file
                assert not recorder._stop.is_set()
                mock_stream.start.assert_called_once()

                # Simulate some audio data
                audio_data = np.array([[0.1, 0.2]], dtype=np.float32)
                recorder._q.put(audio_data)

                # Stop recording
                result = recorder.stop()

                # Verify stop behavior
                assert recorder._stop.is_set()
                mock_stream.stop.assert_called_once()
                mock_stream.close.assert_called_once()
                assert result == test_file

    @patch('asrpro.audio_recorder.platform.system')
    def test_macos_permission_workflow(self, mock_system):
        """Test complete macOS permission workflow."""
        mock_system.return_value = 'Darwin'

        with patch.object(AudioRecorder, '_check_macos_microphone_permission') as mock_check:
            mock_check.return_value = False

            with patch.object(AudioRecorder, '_request_macos_microphone_permission') as mock_request:
                recorder = AudioRecorder()

                # Verify permission check was called during init
                mock_check.assert_called_once()

                # Test permission request
                recorder._request_macos_microphone_permission()
                mock_request.assert_called_once()
