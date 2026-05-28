using System;
using System.IO;
using Windows.Data.Xml.Dom;
using Windows.UI.Notifications;

namespace Deskdrop.Windows
{
    public static class NotificationHelper
    {
        public static void ShowToast(string title, string body, string? iconPath = null)
        {
            try
            {
                XmlDocument xml;
                if (string.IsNullOrEmpty(iconPath) || !File.Exists(iconPath))
                {
                    xml = ToastNotificationManager.GetTemplateContent(ToastTemplateType.ToastText02);
                    xml.GetElementsByTagName("text")[0].AppendChild(xml.CreateTextNode(title));
                    xml.GetElementsByTagName("text")[1].AppendChild(xml.CreateTextNode(body));
                }
                else
                {
                    xml = ToastNotificationManager.GetTemplateContent(ToastTemplateType.ToastImageAndText02);
                    xml.GetElementsByTagName("text")[0].AppendChild(xml.CreateTextNode(title));
                    xml.GetElementsByTagName("text")[1].AppendChild(xml.CreateTextNode(body));
                    var image = xml.GetElementsByTagName("image")[0] as XmlElement;
                    image?.SetAttribute("src", "file:///" + iconPath.Replace("\\", "/"));
                }

                // Make sure to add the app ID to the notifier
                var toast = new ToastNotification(xml);
                ToastNotificationManager.CreateToastNotifier("Deskdrop").Show(toast);
            }
            catch
            {
                // Fallback silently if toast APIs are disabled by user
            }
        }

        public static void ShowToastWithActions(string title, string body, string? iconPath, Action onAccept, Action onReject)
        {
            try
            {
                var xmlString = $@"
<toast>
  <visual>
    <binding template='ToastGeneric'>
      <text>{System.Security.SecurityElement.Escape(title)}</text>
      <text>{System.Security.SecurityElement.Escape(body)}</text>
      {(string.IsNullOrEmpty(iconPath) ? "" : $"<image placement='appLogoOverride' src='file:///{iconPath.Replace("\\", "/")}' />")}
    </binding>
  </visual>
  <actions>
    <action content='Accept' arguments='accept' />
    <action content='Reject' arguments='reject' />
  </actions>
</toast>";

                var xml = new XmlDocument();
                xml.LoadXml(xmlString);

                var toast = new ToastNotification(xml);
                toast.Activated += (sender, args) =>
                {
                    if (args is ToastActivatedEventArgs toastArgs)
                    {
                        if (toastArgs.Arguments == "accept")
                        {
                            System.Windows.Application.Current?.Dispatcher.InvokeAsync(onAccept);
                        }
                        else if (toastArgs.Arguments == "reject")
                        {
                            System.Windows.Application.Current?.Dispatcher.InvokeAsync(onReject);
                        }
                    }
                };

                ToastNotificationManager.CreateToastNotifier("Deskdrop").Show(toast);
            }
            catch
            {
                // Fallback silently if toast APIs are disabled by user
            }
        }
    }
}
