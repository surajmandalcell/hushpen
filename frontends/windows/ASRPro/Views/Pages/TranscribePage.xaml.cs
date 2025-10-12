using System;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using Microsoft.Win32;
using Newtonsoft.Json;

namespace ASRPro.Views.Pages
{
    public partial class TranscribePage : System.Windows.Controls.Page
    {
        private readonly HttpClient _httpClient;
        private const string BackendBaseUrl = "http://localhost:8000";
        private string? _selectedFilePath;
        private Process? _backendProcess;

        public TranscribePage()
        {
            InitializeComponent();
            _httpClient = new HttpClient { Timeout = TimeSpan.FromMinutes(10) };

            Loaded += OnLoaded;
            DropZone.MouseLeftButtonUp += (s, e) => BrowseButton_Click(s, e);
        }

        private async void OnLoaded(object sender, RoutedEventArgs e)
        {
            await StartBackendAsync();
            await Task.Delay(3000);
            await CheckBackendHealthAsync();
        }

        public void ForceBackendStop()
        {
            try { _backendProcess?.Kill(); _backendProcess?.Dispose(); } catch { }
        }

        private Task StartBackendAsync()
        {
            return Task.Run(() =>
            {
                try
                {
                    Dispatcher.Invoke(() =>
                    {
                        ProgressText.Text = "Starting backend server...";
                    });

                    var currentDir = AppDomain.CurrentDomain.BaseDirectory;
                    var projectRoot = Directory.GetParent(currentDir)?.Parent?.Parent?.Parent?.Parent?.FullName;
                    var sidecarPath = Path.Combine(projectRoot ?? "", "sidecar");

                    if (Directory.Exists(sidecarPath))
                    {
                        var mainPyPath = Path.Combine(sidecarPath, "main.py");
                        if (File.Exists(mainPyPath))
                        {
                            _backendProcess = new Process
                            {
                                StartInfo = new ProcessStartInfo
                                {
                                    FileName = "python",
                                    Arguments = "main.py",
                                    WorkingDirectory = sidecarPath,
                                    UseShellExecute = false,
                                    CreateNoWindow = true,
                                    RedirectStandardOutput = true,
                                    RedirectStandardError = true
                                }
                            };

                            _backendProcess.Start();
                            Dispatcher.Invoke(() => ProgressText.Text = "Backend starting...");
                        }
                        else
                        {
                            Dispatcher.Invoke(() => ProgressText.Text = "Backend main.py not found");
                        }
                    }
                    else
                    {
                        Dispatcher.Invoke(() => ProgressText.Text = "Sidecar directory not found");
                    }
                }
                catch (Exception ex)
                {
                    Dispatcher.Invoke(() =>
                    {
                        ProgressText.Text = $"Failed to start backend: {ex.Message}";
                    });
                }
            });
        }

        private async Task CheckBackendHealthAsync()
        {
            try
            {
                ProgressText.Text = "Connecting to backend...";

                var response = await _httpClient.GetAsync($"{BackendBaseUrl}/health");
                if (response.IsSuccessStatusCode)
                {
                    ProgressText.Text = "Backend connected";
                }
                else
                {
                    ProgressText.Text = "Backend error";
                }
            }
            catch
            {
                ProgressText.Text = "Backend not available";
            }
        }

        private void BrowseButton_Click(object sender, RoutedEventArgs e)
        {
            var openFileDialog = new OpenFileDialog
            {
                Title = "Select Audio or Video File",
                Filter = "Media Files|*.mp3;*.wav;*.m4a;*.flac;*.mp4;*.mkv;*.mov;*.avi|All Files|*.*"
            };

            if (openFileDialog.ShowDialog() == true)
            {
                SelectFile(openFileDialog.FileName);
            }
        }

        private void SelectFile(string filePath)
        {
            _selectedFilePath = filePath;
            ProgressText.Text = "File selected - ready to transcribe";
            DropZone.BorderBrush = new SolidColorBrush(Color.FromRgb(59, 91, 219));
            TranscribeButton.IsEnabled = true;
        }

        private async void TranscribeButton_Click(object sender, RoutedEventArgs e)
        {
            if (string.IsNullOrEmpty(_selectedFilePath))
            {
                MessageBox.Show("Please select a file first.", "No File Selected",
                    System.Windows.MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }

            try
            {
                TranscriptionProgress.Visibility = Visibility.Visible;
                ProgressText.Visibility = Visibility.Visible;
                TranscribeButton.IsEnabled = false;
                ProgressText.Text = "Processing file...";

                string audioFilePath = await ConvertToOptimalFormat(_selectedFilePath);

                // For now hardcode model; later expose in Settings
                string model = "whisper-1";
                TranscriptionProgress.Value = 30;

                string result = await TranscribeAudioAsync(audioFilePath, model);
                TranscriptionProgress.Value = 100;
                ProgressText.Text = "Transcription complete!";

                ResultsTextBlock.Text = result;
                CopyButton.IsEnabled = true;
                SaveButton.IsEnabled = true;
            }
            catch (Exception ex)
            {
                MessageBox.Show($"Error during transcription: {ex.Message}", "Transcription Error",
                    System.Windows.MessageBoxButton.OK, MessageBoxImage.Error);
            }
            finally
            {
                TranscribeButton.IsEnabled = true;
                TranscriptionProgress.Visibility = Visibility.Collapsed;
                ProgressText.Visibility = Visibility.Collapsed;
            }
        }

        private async Task<string> ConvertToOptimalFormat(string inputPath)
        {
            string extension = Path.GetExtension(inputPath).ToLower();

            if (extension == ".wav" || extension == ".mp3" || extension == ".m4a" || extension == ".flac")
                return inputPath;

            string outputPath = Path.Combine(Path.GetTempPath(),
                Path.GetFileNameWithoutExtension(inputPath) + "_converted.wav");

            try
            {
                ProgressText.Text = "Converting to optimal audio format...";
                TranscriptionProgress.Value = 10;

                await FFMpegCore.FFMpegArguments
                    .FromFileInput(inputPath)
                    .OutputToFile(outputPath, true, options => options
                        .WithAudioCodec("pcm_s16le")
                        .WithAudioSamplingRate(16000)
                        .WithAudioBitrate(256))
                    .ProcessAsynchronously();

                return outputPath;
            }
            catch (Exception ex)
            {
                throw new Exception($"Failed to convert media file: {ex.Message}");
            }
        }

        private async Task<string> TranscribeAudioAsync(string audioFilePath, string model)
        {
            try
            {
                ProgressText.Text = "Sending to AI model...";
                TranscriptionProgress.Value = 50;

                using var form = new MultipartFormDataContent();
                var fileBytes = await File.ReadAllBytesAsync(audioFilePath);
                var fileContent = new ByteArrayContent(fileBytes)
                {
                    Headers = { ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("audio/wav") }
                };
                form.Add(fileContent, "file", Path.GetFileName(audioFilePath));

                var url = $"{BackendBaseUrl}/v1/audio/transcriptions?model={model}&response_format=json";
                var response = await _httpClient.PostAsync(url, form);

                if (!response.IsSuccessStatusCode)
                {
                    var errorContent = await response.Content.ReadAsStringAsync();
                    throw new Exception($"Backend error ({response.StatusCode}): {errorContent}");
                }

                ProgressText.Text = "Processing response...";
                TranscriptionProgress.Value = 90;

                var jsonResponse = await response.Content.ReadAsStringAsync();
                var transcriptionResult = JsonConvert.DeserializeObject<dynamic>(jsonResponse);

                return transcriptionResult?.text ?? "No transcription text received.";
            }
            catch (Exception ex)
            {
                throw new Exception($"Transcription request failed: {ex.Message}");
            }
        }

        private void CopyButton_Click(object sender, RoutedEventArgs e)
        {
            if (!string.IsNullOrEmpty(ResultsTextBlock.Text))
            {
                Clipboard.SetText(ResultsTextBlock.Text);
                MessageBox.Show("Text copied to clipboard!", "Copy Successful",
                    System.Windows.MessageBoxButton.OK, MessageBoxImage.Information);
            }
        }

        private void SaveButton_Click(object sender, RoutedEventArgs e)
        {
            if (string.IsNullOrEmpty(ResultsTextBlock.Text))
                return;

            var saveFileDialog = new SaveFileDialog
            {
                Title = "Save Transcription",
                Filter = "Text Files|*.txt|All Files|*.*",
                FileName = "transcription.txt"
            };

            if (saveFileDialog.ShowDialog() == true)
            {
                try
                {
                    File.WriteAllText(saveFileDialog.FileName, ResultsTextBlock.Text, Encoding.UTF8);
                    MessageBox.Show("Transcription saved successfully!", "Save Successful",
                        System.Windows.MessageBoxButton.OK, MessageBoxImage.Information);
                }
                catch (Exception ex)
                {
                    MessageBox.Show($"Error saving file: {ex.Message}", "Save Error",
                        System.Windows.MessageBoxButton.OK, MessageBoxImage.Error);
                }
            }
        }

        private void DropZone_DragEnter(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                e.Effects = DragDropEffects.Copy;
                DropZone.BorderBrush = new SolidColorBrush(Color.FromRgb(59, 91, 219));
            }
            else
            {
                e.Effects = DragDropEffects.None;
            }
        }

        private void DropZone_DragLeave(object sender, DragEventArgs e)
        {
            DropZone.BorderBrush = new SolidColorBrush(Color.FromRgb(58, 58, 58));
        }

        private void DropZone_Drop(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                var files = (string[])e.Data.GetData(DataFormats.FileDrop);
                if (files.Length > 0)
                {
                    SelectFile(files[0]);
                }
            }
        }
    }
}
