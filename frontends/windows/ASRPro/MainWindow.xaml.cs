using System;
using System.Threading.Tasks;
using System.Windows;
using Microsoft.Win32;
using System.IO;

namespace ASRPro
{
    /// <summary>
    /// Interaction logic for MainWindow.xaml
    /// </summary>
    public partial class MainWindow : Window
    {
        // Backend API integration will be implemented here
        // private readonly HttpClient _httpClient;
        // private const string BackendBaseUrl = "http://localhost:8000";
        
        public MainWindow()
        {
            InitializeComponent();
            Loaded += MainWindow_Loaded;
        }

        private async void MainWindow_Loaded(object sender, RoutedEventArgs e)
        {
            // Check backend health on startup
            await CheckBackendHealthAsync();
        }

        private async Task CheckBackendHealthAsync()
        {
            try
            {
                // Backend health check will be implemented here
                // var response = await _httpClient.GetAsync($"{BackendBaseUrl}/health");
                // if (response.IsSuccessStatusCode)
                // {
                //     StatusTextBlock.Text = "Connected";
                //     StatusTextBlock.Foreground = System.Windows.Media.Brushes.Green;
                // }
                // else
                // {
                //     StatusTextBlock.Text = "Error";
                //     StatusTextBlock.Foreground = System.Windows.Media.Brushes.Red;
                // }
                
                // Placeholder implementation
                await Task.Delay(1000);
                StatusTextBlock.Text = "Not Connected";
                StatusTextBlock.Foreground = System.Windows.Media.Brushes.Red;
                
                ResultsTextBox.Text = "Backend health check will be implemented here.\n\n" +
                    "The app will check if the backend server is running at http://localhost:8000 " +
                    "before attempting transcription.\n\n" +
                    "Make sure the backend server is running before using this feature.";
            }
            catch (Exception ex)
            {
                StatusTextBlock.Text = "Error";
                StatusTextBlock.Foreground = System.Windows.Media.Brushes.Red;
                ResultsTextBox.Text = $"Error connecting to backend: {ex.Message}";
            }
        }

        private async void TranscribeButton_Click(object sender, RoutedEventArgs e)
        {
            // Show progress overlay
            ProgressOverlay.Visibility = Visibility.Visible;
            ProgressText.Text = "Preparing to transcribe...";
            
            try
            {
                // Backend API integration will be implemented here
                // This will connect to the backend API to transcribe audio files
                
                await Task.Delay(2000); // Simulate processing time
                
                ResultsTextBox.Text = "Audio transcription functionality will be implemented here.\n\n" +
                    "This will:\n" +
                    "1. Connect to the backend API at http://localhost:8000\n" +
                    "2. Send audio files for transcription\n" +
                    "3. Display the transcribed text\n\n" +
                    "Make sure the backend server is running before using this feature.";
            }
            catch (Exception ex)
            {
                MessageBox.Show($"Error during transcription: {ex.Message}", "Error", 
                    MessageBoxButton.OK, MessageBoxImage.Error);
            }
            finally
            {
                // Hide progress overlay
                ProgressOverlay.Visibility = Visibility.Collapsed;
            }
        }

        private void BrowseButton_Click(object sender, RoutedEventArgs e)
        {
            var openFileDialog = new OpenFileDialog
            {
                Title = "Select Audio File",
                Filter = "Audio Files (*.mp3;*.wav;*.m4a;*.flac)|*.mp3;*.wav;*.m4a;*.flac|All Files (*.*)|*.*",
                FilterIndex = 1
            };

            if (openFileDialog.ShowDialog() == true)
            {
                string fileName = openFileDialog.FileName;
                
                // Audio file selection will be implemented here
                // This will validate the file and prepare it for transcription
                
                ResultsTextBox.Text = $"Selected file: {fileName}\n\n" +
                    "Audio file selection functionality will be implemented here.\n\n" +
                    "This will:\n" +
                    "1. Validate the selected audio file\n" +
                    "2. Prepare the file for upload to the backend\n" +
                    "3. Enable the transcribe button if valid";
            }
        }

        private void SettingsButton_Click(object sender, RoutedEventArgs e)
        {
            // Settings window will be implemented here
            MessageBox.Show("Settings functionality will be implemented here.\n\n" +
                "This will include:\n" +
                "- Backend URL configuration\n" +
                "- Audio format preferences\n" +
                "- Transcription language settings\n" +
                "- Output format options", 
                "Settings", MessageBoxButton.OK, MessageBoxImage.Information);
        }
    }
}