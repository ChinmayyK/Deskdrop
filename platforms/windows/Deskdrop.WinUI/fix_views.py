import re

with open('/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/MainWindow.xaml', 'r') as f:
    content = f.read()

def extract_balanced(text, start_tag):
    start_idx = text.find(start_tag)
    if start_idx == -1: return ""
    
    count = 0
    i = start_idx
    while i < len(text):
        if text[i:i+5] == '<Grid' and (i == start_idx or text[i-1] in ' \n\t>'):
            count += 1
            i += 5
        elif text[i:i+7] == '</Grid>':
            count -= 1
            i += 7
            if count == 0:
                return text[start_idx:i]
        else:
            i += 1
    return ""

activity_grid = extract_balanced(content, '<Grid x:Name="ActivityView"')
transfers_grid = extract_balanced(content, '<Grid x:Name="TransfersView"')
devices_grid = extract_balanced(content, '<Grid x:Name="DevicesView"')

def create_uc(name, grid_content):
    uc_content = f"""<UserControl x:Class="Deskdrop.Windows.Views.{name}"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" 
             xmlns:d="http://schemas.microsoft.com/expression/blend/2008" 
             xmlns:iconPacks="http://metro.mahapps.com/winfx/xaml/iconpacks"
             xmlns:local="clr-namespace:Deskdrop.Windows.Views"
             mc:Ignorable="d" 
             d:DesignHeight="450" d:DesignWidth="800">
{grid_content}
</UserControl>"""
    with open(f'/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/Views/{name}.xaml', 'w') as f:
        f.write(uc_content)

create_uc('ActivityView', activity_grid)
create_uc('TransfersView', transfers_grid)
create_uc('DevicesView', devices_grid)
