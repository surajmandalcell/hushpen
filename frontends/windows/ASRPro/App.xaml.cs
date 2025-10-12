using System;
using System.Windows;
using System.Threading;
using System.Runtime.InteropServices;
using Wpf.Ui.Appearance;

namespace ASRPro
{
    /// <summary>
    /// Interaction logic for App.xaml
    /// </summary>
    public partial class App : Application
    {
        private static Mutex? _mutex;
        private static MainWindow? _mainWindow;

        // Import SetConsoleCtrlHandler from kernel32.dll
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool SetConsoleCtrlHandler(ConsoleCtrlDelegate? handlerRoutine, bool add);

        private delegate bool ConsoleCtrlDelegate(CtrlTypes ctrlType);

        private enum CtrlTypes
        {
            CTRL_C_EVENT = 0,
            CTRL_BREAK_EVENT = 1,
            CTRL_CLOSE_EVENT = 2,
            CTRL_LOGOFF_EVENT = 5,
            CTRL_SHUTDOWN_EVENT = 6
        }

        protected override void OnStartup(StartupEventArgs e)
        {
            // Single instance check
            const string mutexName = "ASRPro_SingleInstance";
            _mutex = new Mutex(true, mutexName, out bool isNewInstance);

            if (!isNewInstance)
            {
                // Another instance is already running, exit silently
                Shutdown();
                return;
            }

            // Set up console Ctrl+C handler
            SetConsoleCtrlHandler(ConsoleCtrlHandler, true);

            base.OnStartup(e);

            // Apply WPF UI theme (Dark)
            try { ApplicationThemeManager.Apply(ApplicationTheme.Dark); } catch { }

            // Create main window directly without DI to avoid multiple instances
            _mainWindow = new MainWindow();

            // Start minimized to tray
            _mainWindow.WindowState = WindowState.Minimized;
            _mainWindow.Show();
        }

        private static bool ConsoleCtrlHandler(CtrlTypes ctrlType)
        {
            // Handle Ctrl+C, Ctrl+Break, or console close events
            if (ctrlType == CtrlTypes.CTRL_C_EVENT ||
                ctrlType == CtrlTypes.CTRL_BREAK_EVENT ||
                ctrlType == CtrlTypes.CTRL_CLOSE_EVENT)
            {
                // Signal the main window to exit properly
                Current?.Dispatcher.Invoke(() =>
                {
                    _mainWindow?.ForceExit();
                    Current?.Shutdown();
                });

                // Wait a bit for cleanup
                Thread.Sleep(1000);
                return true;
            }
            return false;
        }

        protected override void OnExit(ExitEventArgs e)
        {
            _mainWindow?.ForceExit();
            _mutex?.Dispose();
            base.OnExit(e);
        }
    }
}
