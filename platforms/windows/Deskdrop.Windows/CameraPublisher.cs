using System;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Threading.Tasks;
using Windows.Graphics.Imaging;
using Windows.Media.Capture;
using Windows.Media.Capture.Frames;

namespace Deskdrop.Windows
{
    public class CameraPublisher : IDisposable
    {
        private readonly ClipboardManager _clipboardManager;
        private MediaCapture? _mediaCapture;
        private MediaFrameReader? _frameReader;
        private bool _isBroadcasting;
        private DateTime _lastFrameTime = DateTime.MinValue;

        public CameraPublisher(ClipboardManager clipboardManager)
        {
            _clipboardManager = clipboardManager;
        }

        public async Task StartBroadcastingAsync()
        {
            if (_isBroadcasting) return;
            _isBroadcasting = true;

            try
            {
                // Find a video capture device
                var frameSourceGroups = await MediaFrameSourceGroup.FindAllAsync();
                var selectedGroup = frameSourceGroups.FirstOrDefault(g => 
                    g.SourceInfos.Any(info => info.MediaStreamType == MediaStreamType.VideoRecord || info.MediaStreamType == MediaStreamType.VideoPreview));

                if (selectedGroup == null)
                {
                    _isBroadcasting = false;
                    throw new Exception("No camera found.");
                }

                _mediaCapture = new MediaCapture();
                var settings = new MediaCaptureInitializationSettings
                {
                    SourceGroup = selectedGroup,
                    SharingMode = MediaCaptureSharingMode.SharedReadOnly,
                    MemoryPreference = MediaCaptureMemoryPreference.Cpu,
                    StreamingCaptureMode = StreamingCaptureMode.Video
                };

                await _mediaCapture.InitializeAsync(settings);

                var sourceInfo = selectedGroup.SourceInfos.FirstOrDefault(info => 
                    info.MediaStreamType == MediaStreamType.VideoRecord || info.MediaStreamType == MediaStreamType.VideoPreview);
                
                if (sourceInfo == null)
                {
                    _isBroadcasting = false;
                    return;
                }

                var frameSource = _mediaCapture.FrameSources[sourceInfo.Id];
                
                // Try to find a reasonable format (e.g. 640x480 at 30fps) to keep bandwidth low
                var format = frameSource.SupportedFormats.FirstOrDefault(f => 
                    f.VideoFormat.Width <= 1280 && f.FrameRate.Numerator / f.FrameRate.Denominator <= 30);
                    
                if (format != null)
                {
                    await frameSource.SetFormatAsync(format);
                }

                _frameReader = await _mediaCapture.CreateFrameReaderAsync(frameSource);
                _frameReader.AcquisitionMode = MediaFrameReaderAcquisitionMode.Realtime;
                _frameReader.FrameArrived += FrameReader_FrameArrived;
                
                var status = await _frameReader.StartAsync();
                if (status != MediaFrameReaderStartStatus.Success)
                {
                    _isBroadcasting = false;
                    throw new Exception($"Could not start frame reader: {status}");
                }
            }
            catch (Exception ex)
            {
                _isBroadcasting = false;
                StopBroadcasting();
                throw new Exception("Failed to start camera: " + ex.Message);
            }
        }

        public void StopBroadcasting()
        {
            _isBroadcasting = false;
            
            if (_frameReader != null)
            {
                _frameReader.FrameArrived -= FrameReader_FrameArrived;
                var _ = _frameReader.StopAsync(); // Fire and forget
                _frameReader.Dispose();
                _frameReader = null;
            }

            if (_mediaCapture != null)
            {
                _mediaCapture.Dispose();
                _mediaCapture = null;
            }
        }

        private async void FrameReader_FrameArrived(MediaFrameReader sender, MediaFrameArrivedEventArgs args)
        {
            if (!_isBroadcasting) return;

            // Throttle to ~15 fps to save bandwidth
            if ((DateTime.Now - _lastFrameTime).TotalMilliseconds < 66) return;
            
            using var frameReference = sender.TryAcquireLatestFrame();
            if (frameReference == null) return;

            var videoFrame = frameReference.VideoMediaFrame;
            if (videoFrame == null) return;
            
            var softwareBitmap = videoFrame.SoftwareBitmap;
            if (softwareBitmap == null)
            {
                // If it's a Direct3D surface, try to get a software bitmap
                var d3dSurface = videoFrame.Direct3DSurface;
                if (d3dSurface != null)
                {
                    softwareBitmap = await SoftwareBitmap.CreateCopyFromSurfaceAsync(d3dSurface);
                }
            }

            if (softwareBitmap != null)
            {
                _lastFrameTime = DateTime.Now;
                
                // Convert to Bgra8 if necessary
                if (softwareBitmap.BitmapPixelFormat != BitmapPixelFormat.Bgra8 || softwareBitmap.BitmapAlphaMode == BitmapAlphaMode.Straight)
                {
                    softwareBitmap = SoftwareBitmap.Convert(softwareBitmap, BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied);
                }

                // Encode to JPEG
                using var stream = new global::Windows.Storage.Streams.InMemoryRandomAccessStream();
                var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.JpegEncoderId, stream);
                // Set quality to 60% to keep size small
                var propertySet = new global::Windows.Foundation.Collections.PropertySet();
                var qualityValue = new BitmapTypedValue(0.6, global::Windows.Foundation.PropertyType.Single);
                propertySet.Add("ImageQuality", qualityValue);
                
                encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.JpegEncoderId, stream, propertySet);
                encoder.SetSoftwareBitmap(softwareBitmap);
                
                try 
                {
                    await encoder.FlushAsync();
                    var bytes = new byte[stream.Size];
                    await stream.AsStreamForRead().ReadAsync(bytes, 0, bytes.Length);
                    
                    // Push via ClipboardManager -> NativeCore
                    _clipboardManager.PushCameraFrame(bytes);
                }
                catch { /* Ignore encoder errors like device lost */ }
                
                softwareBitmap.Dispose();
            }
        }

        public void Dispose()
        {
            StopBroadcasting();
        }
    }
}
