using System;
using System.Windows;
using System.Threading;

namespace ASRPro
{
    /// <summary>
    /// Interaction logic for App.xaml
    /// </summary>
    public partial class App : Application
    {
        private static Mutex? _mutex;

        protected override void OnStartup(StartupEventArgs e)
        {
            // Single instance check
            const string mutexName = "SpokenlyASRPro_SingleInstance";
            _mutex = new Mutex(true, mutexName, out bool isNewInstance);

            if (!isNewInstance)
            {
                // Another instance is already running, exit silently
                Shutdown();
                return;
            }

            base.OnStartup(e);

            // Create main window directly without DI to avoid multiple instances
            var mainWindow = new MainWindow();

            // Start minimized to tray
            mainWindow.WindowState = WindowState.Minimized;
            mainWindow.Show();
        }


        protected override void OnExit(ExitEventArgs e)
        {
            _mutex?.Dispose();
            base.OnExit(e);
        }
    }
}