using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace Deskdrop.Windows
{
    public partial class RemoteExplorerWindow : Window, INotifyPropertyChanged
    {
        private readonly string _deviceId;
        public ObservableCollection<RemoteFile> Files { get; } = new();

        private string _currentPath = "/";
        public string CurrentPath
        {
            get => _currentPath;
            set
            {
                if (_currentPath != value)
                {
                    _currentPath = value;
                    OnPropertyChanged();
                    _ = LoadFilesAsync();
                }
            }
        }

        private bool _isSelectionMode;
        public bool IsSelectionMode
        {
            get => _isSelectionMode;
            set
            {
                if (_isSelectionMode != value)
                {
                    _isSelectionMode = value;
                    OnPropertyChanged();
                    BatchActionBar.Visibility = value ? Visibility.Visible : Visibility.Collapsed;
                    UpdateSelectedCount();
                }
            }
        }

        private string _selectedCountText = "0 items selected";
        public string SelectedCountText
        {
            get => _selectedCountText;
            set
            {
                if (_selectedCountText != value)
                {
                    _selectedCountText = value;
                    OnPropertyChanged();
                }
            }
        }

        public RemoteExplorerWindow(string deviceId, string deviceName)
        {
            InitializeComponent();
            _deviceId = deviceId;
            Title = $"Remote Explorer - {deviceName}";
            DataContext = this;
            
            _ = LoadFilesAsync();
        }

        private async System.Threading.Tasks.Task LoadFilesAsync()
        {
            Dispatcher.Invoke(() =>
            {
                LoadingSpinner.Visibility = Visibility.Visible;
                FileListView.Visibility = Visibility.Collapsed;
            });

            try
            {
                var response = await DaemonClient.RemoteFileListRequestAsync(_deviceId, CurrentPath);
                if (response != null && response.RootElement.TryGetProperty("files", out var filesProp))
                {
                    var files = JsonSerializer.Deserialize<System.Collections.Generic.List<RemoteFile>>(filesProp.GetRawText()) ?? new();
                    Dispatcher.Invoke(() =>
                    {
                        Files.Clear();
                        foreach (var file in files.OrderByDescending(f => f.is_dir).ThenBy(f => f.name))
                        {
                            file.PropertyChanged += File_PropertyChanged;
                            Files.Add(file);
                        }
                    });
                }
            }
            catch (Exception ex)
            {
                Dispatcher.Invoke(() => MessageBox.Show(this, $"Failed to load files: {ex.Message}", "Error"));
            }
            finally
            {
                Dispatcher.Invoke(() =>
                {
                    LoadingSpinner.Visibility = Visibility.Collapsed;
                    FileListView.Visibility = Visibility.Visible;
                });
            }
        }

        private void File_PropertyChanged(object? sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName == nameof(RemoteFile.IsSelected))
            {
                UpdateSelectedCount();
            }
        }

        private void UpdateSelectedCount()
        {
            int count = Files.Count(f => f.IsSelected);
            SelectedCountText = $"{count} item{(count == 1 ? "" : "s")} selected";
        }

        private void BtnBack_Click(object sender, RoutedEventArgs e)
        {
            if (CurrentPath == "/" || string.IsNullOrWhiteSpace(CurrentPath)) return;
            
            var parts = CurrentPath.TrimEnd('/').Split('/');
            if (parts.Length <= 1)
            {
                CurrentPath = "/";
            }
            else
            {
                CurrentPath = string.Join("/", parts.Take(parts.Length - 1));
                if (string.IsNullOrWhiteSpace(CurrentPath)) CurrentPath = "/";
            }
        }

        private void FileListView_MouseDoubleClick(object sender, MouseButtonEventArgs e)
        {
            if (IsSelectionMode) return;
            
            if (FileListView.SelectedItem is RemoteFile file && file.is_dir)
            {
                string sep = CurrentPath.EndsWith("/") ? "" : "/";
                CurrentPath = $"{CurrentPath}{sep}{file.name}";
            }
            else if (FileListView.SelectedItem is RemoteFile regularFile && !regularFile.is_dir)
            {
                // Just download it on double click
                string sep = CurrentPath.EndsWith("/") ? "" : "/";
                string fullPath = $"{CurrentPath}{sep}{regularFile.name}";
                DaemonClient.RemoteFileActionRequest(_deviceId, "download", fullPath);
                MessageBox.Show(this, $"Started download of {regularFile.name}", "Download");
            }
        }

        private void BtnSelectMode_CheckedChanged(object sender, RoutedEventArgs e)
        {
            IsSelectionMode = BtnSelectMode.IsChecked == true;
            if (!IsSelectionMode)
            {
                foreach (var file in Files) file.IsSelected = false;
            }
        }

        private void BtnDownloadBatch_Click(object sender, RoutedEventArgs e)
        {
            var selected = Files.Where(f => f.IsSelected && !f.is_dir).ToList();
            if (selected.Count == 0) return;

            string sep = CurrentPath.EndsWith("/") ? "" : "/";
            foreach (var file in selected)
            {
                string fullPath = $"{CurrentPath}{sep}{file.name}";
                DaemonClient.RemoteFileActionRequest(_deviceId, "download", fullPath);
            }
            
            MessageBox.Show(this, $"Started download for {selected.Count} items.", "Batch Download");
            BtnSelectMode.IsChecked = false; // Exit selection mode
        }

        private void BtnDeleteBatch_Click(object sender, RoutedEventArgs e)
        {
            var selected = Files.Where(f => f.IsSelected).ToList();
            if (selected.Count == 0) return;

            var result = MessageBox.Show(this, $"Are you sure you want to delete {selected.Count} item(s)?", "Confirm Delete", MessageBoxButton.YesNo, MessageBoxImage.Warning);
            if (result == MessageBoxResult.Yes)
            {
                string sep = CurrentPath.EndsWith("/") ? "" : "/";
                foreach (var file in selected)
                {
                    string fullPath = $"{CurrentPath}{sep}{file.name}";
                    DaemonClient.RemoteFileActionRequest(_deviceId, "delete", fullPath);
                }
                
                // Refresh folder
                BtnSelectMode.IsChecked = false;
                _ = LoadFilesAsync();
            }
        }

        public event PropertyChangedEventHandler? PropertyChanged;
        protected void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        }
    }
}
