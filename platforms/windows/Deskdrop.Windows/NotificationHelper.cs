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
    }
}
