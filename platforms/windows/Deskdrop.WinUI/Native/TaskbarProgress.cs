using System;
using System.Runtime.InteropServices;

namespace Deskdrop.WinUI
{
    public static class TaskbarProgress
    {
        public enum TaskbarStates
        {
            NoProgress = 0,
            Indeterminate = 0x1,
            Normal = 0x2,
            Error = 0x4,
            Paused = 0x8
        }

        [ComImportAttribute()]
        [GuidAttribute("ea1afb91-9e28-4b86-90e9-9e9f8a5eefaf")]
        [InterfaceTypeAttribute(ComInterfaceType.InterfaceIsIUnknown)]
        private interface ITaskbarList3
        {
            [PreserveSig] void HrInit();
            [PreserveSig] void AddTab(IntPtr hwnd);
            [PreserveSig] void DeleteTab(IntPtr hwnd);
            [PreserveSig] void ActivateTab(IntPtr hwnd);
            [PreserveSig] void SetActiveAlt(IntPtr hwnd);
            [PreserveSig] void MarkFullscreenWindow(IntPtr hwnd, [MarshalAs(UnmanagedType.Bool)] bool fFullscreen);
            [PreserveSig] void SetProgressValue(IntPtr hwnd, ulong ullCompleted, ulong ullTotal);
            [PreserveSig] void SetProgressState(IntPtr hwnd, TaskbarStates state);
        }

        [ComImportAttribute()]
        [GuidAttribute("56FDF344-FD6D-11d0-958A-006097C9A090")]
        [ClassInterfaceAttribute(ClassInterfaceType.None)]
        private class TaskbarInstance { }

        private static ITaskbarList3? _taskbarInstance;

        public static void SetState(IntPtr hwnd, TaskbarStates state)
        {
            try
            {
                if (_taskbarInstance == null)
                {
                    _taskbarInstance = (ITaskbarList3)new TaskbarInstance();
                    _taskbarInstance.HrInit();
                }
                _taskbarInstance.SetProgressState(hwnd, state);
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        public static void SetValue(IntPtr hwnd, double progress, double total)
        {
            try
            {
                if (_taskbarInstance == null)
                {
                    _taskbarInstance = (ITaskbarList3)new TaskbarInstance();
                    _taskbarInstance.HrInit();
                }
                _taskbarInstance.SetProgressValue(hwnd, (ulong)progress, (ulong)total);
            }
            catch (Exception ex) { App.HandleError(ex); }
        }
    }
}
