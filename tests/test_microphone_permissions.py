"""Test microphone permission handling with comprehensive TDD approach."""

import pytest
import platform
import tempfile
from pathlib import Path
from typing import Any
from unittest.mock import Mock, patch, MagicMock
import subprocess
from asrpro.audio_recorder import AudioRecorder


class TestMicrophonePermissionDetection:
    """Test microphone permission detection functionality."""

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_check_success_on_macos(self, mock_query, mock_system):
        """Test successful permission check on macOS."""
        mock_system.return_value = 'Darwin'
        mock_devices = [
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100},
            {'name': 'USB Microphone', 'max_input_channels': 2, 'default_samplerate': 48000}
        ]
        mock_query.return_value = mock_devices

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == True
        # Should be called twice: once during init, once during test
        assert mock_query.call_count == 2

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_check_no_input_devices(self, mock_query, mock_system):
        """Test permission check when no input devices are found."""
        mock_system.return_value = 'Darwin'
        mock_devices = [
            {'name': 'Built-in Speakers', 'max_input_channels': 0, 'default_samplerate': 44100}
        ]
        mock_query.return_value = mock_devices

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == False
        # Should print warning message

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_check_query_error(self, mock_query, mock_system):
        """Test permission check when device query fails."""
        mock_system.return_value = 'Darwin'
        mock_query.side_effect = Exception("Permission denied")

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == False

    @patch('asrpro.audio_recorder.platform.system')
    def test_permission_check_non_macos(self, mock_system):
        """Test permission check on non-macOS systems."""
        mock_system.return_value = 'Windows'

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == True

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_check_mixed_devices(self, mock_query, mock_system):
        """Test permission check with mixed input/output devices."""
        mock_system.return_value = 'Darwin'
        mock_devices = [
            {'name': 'Built-in Speakers', 'max_input_channels': 0, 'default_samplerate': 44100},
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100},
            {'name': 'USB Speakers', 'max_input_channels': 0, 'default_samplerate': 48000},
            {'name': 'USB Microphone', 'max_input_channels': 2, 'default_samplerate': 48000}
        ]
        mock_query.return_value = mock_devices

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == True


class TestMicrophonePermissionRequest:
    """Test microphone permission request functionality."""

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.subprocess.run')
    def test_permission_request_macos_success(self, mock_subprocess, mock_system):
        """Test successful permission request on macOS."""
        mock_system.return_value = 'Darwin'
        mock_subprocess.return_value = Mock(returncode=0)

        recorder = AudioRecorder()
        recorder._request_macos_microphone_permission()

        assert mock_subprocess.call_count == 2
        calls = mock_subprocess.call_args_list
        assert 'System Settings' in str(calls[0])
        assert 'Privacy_Microphone' in str(calls[0])

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.subprocess.run')
    def test_permission_request_macos_subprocess_error(self, mock_subprocess, mock_system):
        """Test permission request when subprocess fails."""
        mock_system.return_value = 'Darwin'
        mock_subprocess.side_effect = subprocess.SubprocessError("Failed to execute")

        recorder = AudioRecorder()
        # Should not raise an exception
        recorder._request_macos_microphone_permission()

    @patch('asrpro.audio_recorder.platform.system')
    def test_permission_request_non_macos(self, mock_system):
        """Test permission request on non-macOS systems."""
        mock_system.return_value = 'Windows'

        recorder = AudioRecorder()
        recorder._request_macos_microphone_permission()

        # Should not call subprocess on non-macOS

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.subprocess.run')
    def test_permission_request_command_arguments(self, mock_subprocess, mock_system):
        """Test that correct osascript commands are used."""
        mock_system.return_value = 'Darwin'

        recorder = AudioRecorder()
        recorder._request_macos_microphone_permission()

        # Verify the exact commands
        calls = mock_subprocess.call_args_list
        first_call = calls[0][0][0]  # Get the command list
        second_call = calls[1][0][0]

        assert first_call == ['osascript', '-e', 'tell application "System Settings" to reveal anchor "Privacy_Microphone" of pane id "com.apple.preference.security"']
        assert second_call == ['osascript', '-e', 'tell application "System Settings" to activate']


class TestPermissionHandlingDuringRecording:
    """Test permission handling during recording operations."""

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_recording_with_permission_denied_error(self, mock_query, mock_system):
        """Test recording when permission is denied."""
        mock_system.return_value = 'Darwin'
        mock_query.return_value = [
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100}
        ]

        # Create recorder after mocking to avoid double initialization
        with patch.object(AudioRecorder, '_check_macos_microphone_permission') as mock_check:
            recorder = AudioRecorder()
            test_file = Path("/tmp/test.wav")

            with patch('asrpro.audio_recorder.sd.InputStream') as mock_stream_class:
                # Import sd to use the correct exception type
                import sounddevice as sd
                # Simulate permission denied error
                mock_stream_class.side_effect = sd.PortAudioError("Permission denied")

                with patch.object(recorder, '_request_macos_microphone_permission') as mock_request:
                    with pytest.raises(RuntimeError, match="Microphone access denied"):
                        recorder.start(test_file)

                    mock_request.assert_called_once()

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_recording_with_device_not_found_error(self, mock_query, mock_system):
        """Test recording when device is not found (not permission issue)."""
        mock_system.return_value = 'Darwin'
        mock_query.return_value = [
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100}
        ]

        recorder = AudioRecorder()
        test_file = Path("/tmp/test.wav")

        with patch('asrpro.audio_recorder.sd.InputStream') as mock_stream_class:
            # Simulate device not found error
            mock_stream_class.side_effect = Exception("Device not found")

            with patch.object(recorder, '_request_macos_microphone_permission') as mock_request:
                with pytest.raises(Exception, match="Device not found"):
                    recorder.start(test_file)

                # Should not request permission for device not found
                mock_request.assert_not_called()

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_recording_with_unauthorized_error(self, mock_query, mock_system):
        """Test recording with unauthorized error message."""
        mock_system.return_value = 'Darwin'
        mock_query.return_value = [
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100}
        ]

        # Create recorder after mocking to avoid double initialization
        with patch.object(AudioRecorder, '_check_macos_microphone_permission') as mock_check:
            recorder = AudioRecorder()
            test_file = Path("/tmp/test.wav")

            with patch('asrpro.audio_recorder.sd.InputStream') as mock_stream_class:
                # Import sd to use the correct exception type
                import sounddevice as sd
                # Simulate unauthorized error
                mock_stream_class.side_effect = sd.PortAudioError("Unauthorized access")

                with patch.object(recorder, '_request_macos_microphone_permission') as mock_request:
                    with pytest.raises(RuntimeError, match="Microphone access denied"):
                        recorder.start(test_file)

                    mock_request.assert_called_once()


class TestPermissionRecoveryWorkflow:
    """Test permission recovery and retry workflows."""

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_check_after_denial(self, mock_query, mock_system):
        """Test that permission checking works after initial denial."""
        mock_system.return_value = 'Darwin'

        # First call: no devices (permission denied)
        mock_query.return_value = [
            {'name': 'Built-in Speakers', 'max_input_channels': 0, 'default_samplerate': 44100}
        ]

        recorder = AudioRecorder()
        result1 = recorder._check_macos_microphone_permission()
        assert result1 == False

        # Second call: devices available (permission granted)
        mock_query.return_value = [
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100}
        ]

        result2 = recorder._check_macos_microphone_permission()
        assert result2 == True

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_state_persistence(self, mock_query, mock_system):
        """Test that permission state is properly tracked."""
        mock_system.return_value = 'Darwin'
        mock_query.return_value = [
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100}
        ]

        # Mock the permission check during initialization
        with patch.object(AudioRecorder, '_check_macos_microphone_permission'):
            recorder = AudioRecorder()

        # Multiple checks should return consistent results
        result1 = recorder._check_macos_microphone_permission()
        result2 = recorder._check_macos_microphone_permission()

        assert result1 == True
        assert result2 == True
        assert mock_query.call_count == 2  # Twice during test (init was mocked)


class TestPermissionIntegration:
    """Test integration of permission handling with other components."""

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_initialization_with_permission_check(self, mock_query, mock_system):
        """Test that initialization includes permission check on macOS."""
        mock_system.return_value = 'Darwin'
        mock_query.return_value = [
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100}
        ]

        with patch.object(AudioRecorder, '_check_macos_microphone_permission') as mock_check:
            recorder = AudioRecorder()
            mock_check.assert_called_once()

    @patch('asrpro.audio_recorder.platform.system')
    def test_initialization_skips_permission_check_non_macos(self, mock_system):
        """Test that initialization skips permission check on non-macOS."""
        mock_system.return_value = 'Windows'

        with patch.object(AudioRecorder, '_check_macos_microphone_permission') as mock_check:
            recorder = AudioRecorder()
            mock_check.assert_not_called()

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_device_listing_with_permission_check(self, mock_query, mock_system):
        """Test that device listing respects permission status."""
        mock_system.return_value = 'Darwin'
        mock_query.return_value = [
            {'name': 'Built-in Microphone', 'max_input_channels': 1, 'default_samplerate': 44100}
        ]

        devices: list[dict[str, Any]] = AudioRecorder.list_devices()

        assert len(devices) == 1
        assert devices[0]['name'] == 'Built-in Microphone'
        mock_query.assert_called_once()


class TestPermissionErrorMessages:
    """Test that appropriate error messages are shown."""

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    @patch('builtins.print')
    def test_permission_denied_warning_message(self, mock_print, mock_query, mock_system):
        """Test that appropriate warning is printed when permission is denied."""
        mock_system.return_value = 'Darwin'
        mock_query.return_value = [
            {'name': 'Built-in Speakers', 'max_input_channels': 0, 'default_samplerate': 44100}
        ]

        recorder = AudioRecorder()
        recorder._check_macos_microphone_permission()

        # Check that warning message was printed
        print_calls = [call[0][0] for call in mock_print.call_args_list]
        warning_calls = [call for call in print_calls if 'No input devices found' in call]
        assert len(warning_calls) > 0

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    @patch('builtins.print')
    def test_query_error_message(self, mock_print, mock_query, mock_system):
        """Test that error message is printed when query fails."""
        mock_system.return_value = 'Darwin'
        mock_query.side_effect = Exception("Permission denied")

        recorder = AudioRecorder()
        recorder._check_macos_microphone_permission()

        # Check that error message was printed
        print_calls = [call[0][0] for call in mock_print.call_args_list]
        error_calls = [call for call in print_calls if 'Failed to query audio devices' in call]
        assert len(error_calls) > 0

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.subprocess.run')
    @patch('builtins.print')
    def test_subprocess_error_message(self, mock_print, mock_subprocess, mock_system):
        """Test that error message is printed when subprocess fails."""
        mock_system.return_value = 'Darwin'
        mock_subprocess.side_effect = subprocess.SubprocessError("Failed to execute")

        recorder = AudioRecorder()
        recorder._request_macos_microphone_permission()

        # Check that error message was printed
        print_calls = [call[0][0] for call in mock_print.call_args_list]
        error_calls = [call for call in print_calls if 'Failed to open System Settings' in call]
        assert len(error_calls) > 0


class TestPermissionEdgeCases:
    """Test edge cases and unusual scenarios."""

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_check_empty_device_list(self, mock_query, mock_system):
        """Test permission check with empty device list."""
        mock_system.return_value = 'Darwin'
        mock_query.return_value = []

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == False

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_check_partial_device_info(self, mock_query, mock_system):
        """Test permission check with incomplete device information."""
        mock_system.return_value = 'Darwin'
        mock_devices = [
            {'name': 'Device 1'},  # Missing max_input_channels
            {'max_input_channels': 1},  # Missing name
            {'name': 'Device 3', 'max_input_channels': 1}
        ]
        mock_query.return_value = mock_devices

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        # Should still work, finding the complete device
        assert result == True

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_check_zero_channels(self, mock_query, mock_system):
        """Test permission check with devices that have zero channels."""
        mock_system.return_value = 'Darwin'
        mock_devices = [
            {'name': 'Device 1', 'max_input_channels': 0},
            {'name': 'Device 2', 'max_input_channels': 0}
        ]
        mock_query.return_value = mock_devices

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        assert result == False

    @patch('asrpro.audio_recorder.platform.system')
    @patch('asrpro.audio_recorder.sd.query_devices')
    def test_permission_check_negative_channels(self, mock_query, mock_system):
        """Test permission check with devices that have negative channels."""
        mock_system.return_value = 'Darwin'
        mock_devices = [
            {'name': 'Device 1', 'max_input_channels': -1},
            {'name': 'Device 2', 'max_input_channels': 1}
        ]
        mock_query.return_value = mock_devices

        recorder = AudioRecorder()
        result = recorder._check_macos_microphone_permission()

        # Should still find the valid device
        assert result == True
