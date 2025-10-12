using System;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Input;
using WinForms = System.Windows.Forms;
using Application = System.Windows.Application;
using ASRPro.Views.Pages;

namespace ASRPro
{
    public partial class MainWindow : Wpf.Ui.Controls.FluentWindow
    {
        private WinForms.NotifyIcon? _notifyIcon;
        private bool _isExiting = false;
        private TranscribePage? _transcribePage;

        public MainWindow()
        {
            InitializeComponent();
            Loaded += MainWindow_Loaded;
            StateChanged += MainWindow_StateChanged;
            Closing += MainWindow_Closing;
            InitializeTrayIcon();
            // Backdrop styling can be added if needed; kept minimal for now
        }

        private void MainWindow_Loaded(object sender, RoutedEventArgs e)
        {
            _transcribePage = new TranscribePage();
            ContentFrame.Content = _transcribePage;
        }

        private void BtnTranscribe_Click(object sender, RoutedEventArgs e)
        {
            _transcribePage ??= new TranscribePage();
            ContentFrame.Content = _transcribePage;
        }

        private void BtnSettings_Click(object sender, RoutedEventArgs e)
        {
            ContentFrame.Content = new System.Windows.Controls.Page { Content = new System.Windows.Controls.TextBlock { Text = "Settings (coming soon)", Margin = new Thickness(24) } };
        }

        private void BtnDocs_Click(object sender, RoutedEventArgs e)
        {
            try { Process.Start(new ProcessStartInfo("http://localhost:8000/docs") { UseShellExecute = true }); } catch { }
        }

        private void InitializeTrayIcon()
        {
            if (_notifyIcon != null) return; // Prevent duplicate icons
            try
            {
                var iconPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "icon.png");
                System.Drawing.Icon icon = File.Exists(iconPath)
                    ? System.Drawing.Icon.FromHandle(new System.Drawing.Bitmap(iconPath).GetHicon())
                    : System.Drawing.SystemIcons.Application;

                _notifyIcon = new WinForms.NotifyIcon
                {
                    Icon = icon,
                    Visible = true,
                    Text = "ASRPro"
                };

                var contextMenu = new WinForms.ContextMenuStrip();
                contextMenu.Items.Add("Open ASRPro", null, (s, e) => ShowWindow());
                contextMenu.Items.Add("-");
                contextMenu.Items.Add("Exit", null, (s, e) => ExitApplication());
                _notifyIcon.ContextMenuStrip = contextMenu;
                _notifyIcon.DoubleClick += (s, e) => ShowWindow();
            }
            catch
            {
                _notifyIcon = new WinForms.NotifyIcon
                {
                    Icon = System.Drawing.SystemIcons.Application,
                    Visible = true,
                    Text = "ASRPro"
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
            ForceExit();
            Application.Current.Shutdown();
        }

        public void ForceExit()
        {
            if (_isExiting) return;
            _isExiting = true;

            if (_notifyIcon != null)
            {
                _notifyIcon.Visible = false;
                _notifyIcon.Dispose();
                _notifyIcon = null;
            }

            _transcribePage?.ForceBackendStop();
        }

        private void MainWindow_StateChanged(object? sender, EventArgs e)
        {
            if (WindowState == WindowState.Minimized)
            {
                Hide();
                ShowInTaskbar = false;
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

        protected override void OnClosed(EventArgs e)
        {
            ForceExit();
            base.OnClosed(e);
        }
    }
}
