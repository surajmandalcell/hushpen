using System;
using System.IO;
using System.Net.Http;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using Microsoft.Win32;
using Newtonsoft.Json;
using System.Text;
using FFMpegCore;
using FFMpegCore.Enums;
using System.Diagnostics;
using System.Windows.Input;
using System.Threading;
using WinForms = System.Windows.Forms;
using Application = System.Windows.Application;
using MessageBox = System.Windows.MessageBox;
using Color = System.Windows.Media.Color;

namespace ASRPro
{
    public partial class MainWindow : Window
    {
        private readonly HttpClient _httpClient;
        private const string BackendBaseUrl = "http://localhost:8000";
        private string? _selectedFilePath;
        private WinForms.NotifyIcon? _notifyIcon;
        private bool _isExiting = false;
        private Process? _backendProcess;

        public MainWindow()
        {
            InitializeComponent();
            _httpClient = new HttpClient();
            _httpClient.Timeout = TimeSpan.FromMinutes(10); // Allow longer for transcription
            Loaded += MainWindow_Loaded;
            StateChanged += MainWindow_StateChanged;
            Closing += MainWindow_Closing;

            // Make drop zone clickable
            DropZone.MouseLeftButtonUp += (s, e) => BrowseButton_Click(s, e);

            // Initialize tray icon
            InitializeTrayIcon();
        }

        private async void MainWindow_Loaded(object sender, RoutedEventArgs e)
        {
            // Start backend automatically
            await StartBackendAsync();

            // Wait a moment for backend to start, then check health
            await Task.Delay(3000);
            await CheckBackendHealthAsync();
        }

        private Task StartBackendAsync()
        {
            return Task.Run(() =>
            {
                try
                {
                    Dispatcher.Invoke(() =>
                    {
                        StatusText.Text = "Starting backend server...";
                        StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(255, 189, 46)); // Yellow
                    });

                    // Find the sidecar directory (go up from current directory)
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
                            Dispatcher.Invoke(() => StatusText.Text = "Backend starting...");
                        }
                        else
                        {
                            Dispatcher.Invoke(() => StatusText.Text = "Backend main.py not found");
                        }
                    }
                    else
                    {
                        Dispatcher.Invoke(() => StatusText.Text = "Sidecar directory not found");
                    }
                }
                catch (Exception ex)
                {
                    Dispatcher.Invoke(() =>
                    {
                        StatusText.Text = $"Failed to start backend: {ex.Message}";
                        StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(239, 68, 68)); // Red
                    });
                }
            });
        }

        private async Task CheckBackendHealthAsync()
        {
            try
            {
                StatusText.Text = "Connecting to backend...";
                StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(255, 189, 46)); // Yellow

                var response = await _httpClient.GetAsync($"{BackendBaseUrl}/health");
                if (response.IsSuccessStatusCode)
                {
                    StatusText.Text = "Backend connected";
                    StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(34, 197, 94)); // Green
                }
                else
                {
                    StatusText.Text = "Backend error";
                    StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(239, 68, 68)); // Red
                }
            }
            catch (Exception)
            {
                StatusText.Text = "Backend not available";
                StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(239, 68, 68)); // Red
            }
        }

        private void BrowseButton_Click(object sender, RoutedEventArgs e)
        {
            var openFileDialog = new OpenFileDialog
            {
                Title = "Select Audio or Video File",
                Filter = "Media Files|*.mp4;*.avi;*.mkv;*.mov;*.mp3;*.wav;*.m4a;*.flac;*.ogg;*.aac;*.wma;*.opus;*.webm;*.3gp;*.wmv;*.mpg;*.mpeg;*.m4v;*.ts|" +
                        "Video Files|*.mp4;*.avi;*.mkv;*.mov;*.webm;*.3gp;*.wmv;*.mpg;*.mpeg;*.m4v;*.ts|" +
                        "Audio Files|*.mp3;*.wav;*.m4a;*.flac;*.ogg;*.aac;*.wma;*.opus|" +
                        "All Files|*.*",
                FilterIndex = 1
            };

            if (openFileDialog.ShowDialog() == true)
            {
                SelectFile(openFileDialog.FileName);
            }
        }

        private void SelectFile(string filePath)
        {
            _selectedFilePath = filePath;

            // Update UI
            FileInfoPanel.Visibility = Visibility.Visible;
            FileNameText.Text = Path.GetFileName(filePath);

            var fileInfo = new FileInfo(filePath);
            FileSizeText.Text = $"{fileInfo.Length / (1024 * 1024):F1} MB";

            // Enable transcribe button
            TranscribeButton.IsEnabled = true;
            StatusText.Text = "File selected - ready to transcribe";
            StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(59, 91, 219)); // Blue

            // Update drop zone appearance
            DropText.Text = "✓";
            DropLabel.Text = "File selected";
            DropZone.BorderBrush = new SolidColorBrush(Color.FromRgb(59, 91, 219));
        }

        private async void TranscribeButton_Click(object sender, RoutedEventArgs e)
        {
            if (string.IsNullOrEmpty(_selectedFilePath))
            {
                MessageBox.Show("Please select a file first.", "No File Selected",
                    MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }

            try
            {
                // Show progress
                TranscriptionProgress.Visibility = Visibility.Visible;
                ProgressText.Visibility = Visibility.Visible;
                TranscribeButton.IsEnabled = false;
                StatusText.Text = "Processing file...";
                StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(255, 189, 46)); // Yellow

                // Convert to optimal audio format if needed
                string audioFilePath = await ConvertToOptimalFormat(_selectedFilePath);

                // Update progress
                ProgressText.Text = "Uploading to backend...";
                TranscriptionProgress.Value = 30;

                // Get selected model
                var selectedModel = ((ComboBoxItem)ModelComboBox.SelectedItem)?.Content?.ToString() ?? "whisper-base";

                // Transcribe
                var result = await TranscribeAudioAsync(audioFilePath, selectedModel);

                // Update progress
                TranscriptionProgress.Value = 100;
                ProgressText.Text = "Transcription complete!";

                // Display results
                ResultsTextBlock.Text = result;

                // Enable export buttons
                CopyButton.IsEnabled = true;
                SaveButton.IsEnabled = true;

                StatusText.Text = "Transcription completed";
                StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(34, 197, 94)); // Green

                // Clean up temporary file if created
                if (audioFilePath != _selectedFilePath && File.Exists(audioFilePath))
                {
                    File.Delete(audioFilePath);
                }
            }
            catch (Exception ex)
            {
                MessageBox.Show($"Error during transcription: {ex.Message}", "Transcription Error",
                    MessageBoxButton.OK, MessageBoxImage.Error);

                StatusText.Text = "Transcription failed";
                StatusIndicator.Fill = new SolidColorBrush(Color.FromRgb(239, 68, 68)); // Red
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

            // If already in a good audio format, return as-is
            if (extension == ".wav" || extension == ".mp3" || extension == ".m4a" || extension == ".flac")
            {
                return inputPath;
            }

            // Convert to WAV for optimal quality
            string outputPath = Path.Combine(Path.GetTempPath(),
                Path.GetFileNameWithoutExtension(inputPath) + "_converted.wav");

            try
            {
                ProgressText.Text = "Converting to optimal audio format...";
                TranscriptionProgress.Value = 10;

                await FFMpegArguments
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

                // Add file
                var fileBytes = await File.ReadAllBytesAsync(audioFilePath);
                var fileContent = new ByteArrayContent(fileBytes);
                fileContent.Headers.ContentType = new System.Net.Http.Headers.MediaTypeHeaderValue("audio/wav");
                form.Add(fileContent, "file", Path.GetFileName(audioFilePath));

                // Make request
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
                    MessageBoxButton.OK, MessageBoxImage.Information);
            }
        }

        private void SaveButton_Click(object sender, RoutedEventArgs e)
        {
            if (string.IsNullOrEmpty(ResultsTextBlock.Text))
                return;

            var saveFileDialog = new SaveFileDialog
            {
                Title = "Save Transcription",
                Filter = "Text Files (*.txt)|*.txt|All Files (*.*)|*.*",
                DefaultExt = ".txt",
                FileName = $"transcription_{DateTime.Now:yyyyMMdd_HHmmss}.txt"
            };

            if (saveFileDialog.ShowDialog() == true)
            {
                try
                {
                    File.WriteAllText(saveFileDialog.FileName, ResultsTextBlock.Text, Encoding.UTF8);
                    MessageBox.Show("Transcription saved successfully!", "Save Successful",
                        MessageBoxButton.OK, MessageBoxImage.Information);
                }
                catch (Exception ex)
                {
                    MessageBox.Show($"Error saving file: {ex.Message}", "Save Error",
                        MessageBoxButton.OK, MessageBoxImage.Error);
                }
            }
        }

        private void DocsButton_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                var url = $"{BackendBaseUrl}/docs";
                Process.Start(new ProcessStartInfo(url) { UseShellExecute = true });
            }
            catch (Exception ex)
            {
                MessageBox.Show($"Error opening documentation: {ex.Message}", "Error",
                    MessageBoxButton.OK, MessageBoxImage.Error);
            }
        }

        private void NavButton_Click(object sender, RoutedEventArgs e)
        {
            // Placeholder for navigation functionality
            var button = sender as Button;
            MessageBox.Show($"Navigation to '{button?.Content}' not implemented yet.", "Info",
                MessageBoxButton.OK, MessageBoxImage.Information);
        }

        // Drag and Drop Handlers
        private void DropZone_DragEnter(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                e.Effects = DragDropEffects.Copy;
                DropZone.BorderBrush = new SolidColorBrush(Color.FromRgb(59, 91, 219));
                DropLabel.Text = "Drop file here";
            }
            else
            {
                e.Effects = DragDropEffects.None;
            }
        }

        private void DropZone_DragLeave(object sender, DragEventArgs e)
        {
            DropZone.BorderBrush = new SolidColorBrush(Color.FromRgb(58, 58, 58));
            if (string.IsNullOrEmpty(_selectedFilePath))
            {
                DropLabel.Text = "Drop files here or click to browse";
            }
        }

        private void DropZone_Drop(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                string[] files = (string[])e.Data.GetData(DataFormats.FileDrop);
                if (files.Length > 0)
                {
                    SelectFile(files[0]);
                }
            }
        }

        private void InitializeTrayIcon()
        {
            if (_notifyIcon != null) return; // Prevent duplicate icons

            try
            {
                // Load custom icon - convert PNG to Icon
                var iconPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "icon.png");
                System.Drawing.Icon icon;

                if (File.Exists(iconPath))
                {
                    using var bitmap = new System.Drawing.Bitmap(iconPath);
                    var handle = bitmap.GetHicon();
                    icon = System.Drawing.Icon.FromHandle(handle);
                }
                else
                {
                    icon = System.Drawing.SystemIcons.Application;
                }

                _notifyIcon = new WinForms.NotifyIcon
                {
                    Icon = icon,
                    Visible = true,
                    Text = "Spokenly - ASR Pro"
                };

                // Create context menu
                var contextMenu = new WinForms.ContextMenuStrip();
                contextMenu.Items.Add("Open Spokenly", null, (s, e) => ShowWindow());
                contextMenu.Items.Add("-"); // Separator
                contextMenu.Items.Add("Exit", null, (s, e) => ExitApplication());
                _notifyIcon.ContextMenuStrip = contextMenu;

                // Double-click to show window
                _notifyIcon.DoubleClick += (s, e) => ShowWindow();
            }
            catch (Exception)
            {
                // Fallback to system icon if custom icon fails
                _notifyIcon = new WinForms.NotifyIcon
                {
                    Icon = System.Drawing.SystemIcons.Application,
                    Visible = true,
                    Text = "Spokenly - ASR Pro"
                };
            }
        }

        private void ShowWindow()
        {
            Show();
            WindowState = WindowState.Normal;
            ShowInTaskbar = true;
            Activate();
        }

        private void ExitApplication()
        {
            _isExiting = true;

            // Close backend process
            try
            {
                _backendProcess?.Kill();
                _backendProcess?.Dispose();
            }
            catch { /* Ignore errors when closing backend */ }

            _notifyIcon?.Dispose();
            Application.Current.Shutdown();
        }

        private void MainWindow_StateChanged(object? sender, EventArgs e)
        {
            if (WindowState == WindowState.Minimized)
            {
                Hide();
                ShowInTaskbar = false;
                // Removed annoying notification
            }
            else if (WindowState == WindowState.Normal)
            {
                ShowInTaskbar = true;
            }
        }

        private void MainWindow_Closing(object? sender, System.ComponentModel.CancelEventArgs e)
        {
            if (!_isExiting)
            {
                e.Cancel = true;
                WindowState = WindowState.Minimized;
            }
        }

        // Window Control Handlers
        private void CloseButton_Click(object sender, RoutedEventArgs e)
        {
            WindowState = WindowState.Minimized; // Minimize to tray instead of closing
        }

        private void MinimizeButton_Click(object sender, RoutedEventArgs e)
        {
            WindowState = WindowState.Minimized;
        }

        private void MaximizeButton_Click(object sender, RoutedEventArgs e)
        {
            // Maximize functionality is disabled for this app
            // WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized;
        }

        private void HeaderBorder_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ClickCount == 2)
            {
                // Double-click to maximize/restore (optional)
                // MaximizeButton_Click(sender, new RoutedEventArgs());
            }
            else
            {
                // Single click to drag window
                DragMove();
            }
        }

        protected override void OnClosed(EventArgs e)
        {
            try
            {
                _backendProcess?.Kill();
                _backendProcess?.Dispose();
            }
            catch { /* Ignore errors when closing backend */ }

            _notifyIcon?.Dispose();
            _httpClient?.Dispose();
            base.OnClosed(e);
        }
    }
}