using System;
using System.IO;
using Windows.Data.Xml.Dom;
using Windows.UI.Notifications;

namespace Deskdrop.Windows
{
    public static class NotificationHelper
    {
        public const string AppUserModelID = "Deskdrop.App.1";

        public static void ShowToast(string title, string body, string? iconPath = null, string? launchArg = null)
        {
            try
            {
                var xmlString = $@"
<toast launch='{System.Security.SecurityElement.Escape(launchArg ?? "deskdrop://")}'>
  <visual>
    <binding template='ToastGeneric'>
      <text>{System.Security.SecurityElement.Escape(title)}</text>
      <text>{System.Security.SecurityElement.Escape(body)}</text>
      {(string.IsNullOrEmpty(iconPath) ? "" : $"<image placement='appLogoOverride' src='file:///{iconPath.Replace("\\", "/")}' />")}
    </binding>
  </visual>
</toast>";

                var xml = new XmlDocument();
                xml.LoadXml(xmlString);

                var toast = new ToastNotification(xml);
                ToastNotificationManager.CreateToastNotifier(AppUserModelID).Show(toast);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Toast failed: {ex.Message}");
            }
        }

        public static void ShowToastWithActions(string title, string body, string? iconPath, string acceptUrl, string rejectUrl)
        {
            try
            {
                var xmlString = $@"
<toast launch='deskdrop://'>
  <visual>
    <binding template='ToastGeneric'>
      <text>{System.Security.SecurityElement.Escape(title)}</text>
      <text>{System.Security.SecurityElement.Escape(body)}</text>
      {(string.IsNullOrEmpty(iconPath) ? "" : $"<image placement='appLogoOverride' src='file:///{iconPath.Replace("\\", "/")}' />")}
    </binding>
  </visual>
  <actions>
    <action content='Accept' arguments='{System.Security.SecurityElement.Escape(acceptUrl)}' activationType='protocol' />
    <action content='Reject' arguments='{System.Security.SecurityElement.Escape(rejectUrl)}' activationType='protocol' />
  </actions>
</toast>";

                var xml = new XmlDocument();
                xml.LoadXml(xmlString);

                var toast = new ToastNotification(xml);
                ToastNotificationManager.CreateToastNotifier(AppUserModelID).Show(toast);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"Toast with actions failed: {ex.Message}");
            }
        }
    }
}
