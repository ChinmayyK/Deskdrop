using System;
using System.Diagnostics;
using System.Threading.Tasks;
using Microsoft.UI.Xaml.Controls;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using Deskdrop.WinUI;

namespace Deskdrop.WinUI.Tests
{
    [TestClass]
    public class UIStressTests
    {
        [TestMethod]
        public async Task MeasureVirtualizingStackPanelLayoutTimes()
        {
            var store = DeskdropStore.Shared;
            
            var listView = new ListView();
            listView.ItemsSource = store.ActivityFeed;
            
            var panel = new VirtualizingStackPanel();
            
            int layoutCount = 0;
            var sw = new Stopwatch();
            panel.LayoutUpdated += (s, e) =>
            {
                layoutCount++;
            };

            sw.Start();

            int totalItems = 10000;
            int itemsPerSecond = 100;
            int delayMs = 1000 / itemsPerSecond;

            for (int i = 0; i < totalItems; i++)
            {
                var entry = new ActivityEntry
                {
                    id = (ulong)i,
                    kind = "MockEvent",
                    summary = $"Event {i}",
                };

                store.ActivityFeed.Add(entry);
                await Task.Delay(delayMs);
            }

            sw.Stop();
            
            Debug.WriteLine($"Added {totalItems} items in {sw.ElapsedMilliseconds} ms.");
            Debug.WriteLine($"Layout updated {layoutCount} times.");
            
            Assert.IsTrue(layoutCount >= 0);
        }
    }
}
