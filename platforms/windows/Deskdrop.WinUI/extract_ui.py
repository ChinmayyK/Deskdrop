import re
import os

xaml_path = '/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/MainWindow.xaml'
cs_path = '/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs'
views_dir = '/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/Views'

with open(xaml_path, 'r') as f:
    xaml_content = f.read()

# 1. Extract Resources
resources_start = xaml_content.find('<Window.Resources>') + len('<Window.Resources>')
resources_end = xaml_content.find('</Window.Resources>')
resources_content = xaml_content[resources_start:resources_end]

app_styles_content = f"""<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
                    xmlns:iconPacks="http://metro.mahapps.com/winfx/xaml/iconpacks"
                    xmlns:local="clr-namespace:Deskdrop.Windows">
{resources_content}
</ResourceDictionary>"""

with open('/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/AppStyles.xaml', 'w') as f:
    f.write(app_styles_content)

new_resources = """    <Window.Resources>
        <ResourceDictionary Source="AppStyles.xaml" />
    </Window.Resources>"""

xaml_content = xaml_content[:xaml_content.find('<Window.Resources>')] + new_resources + xaml_content[resources_end+len('</Window.Resources>'):]

# 2. Extract Grids
def extract_grid(name):
    global xaml_content
    start_tag = f'<Grid x:Name="{name}"'
    end_tag = '<!-- SETTINGS VIEW -->' if name == 'DevicesView' else f'<!-- {name.replace("View", "").upper()}S VIEW -->' if name == 'ActivityView' else '<!-- DEVICES VIEW -->'
    
    start_idx = xaml_content.find(start_tag)
    end_idx = xaml_content.find(end_tag) - 17 # approx where previous grid ends
    
    # Just to be safe, find the last </Grid> before end_tag
    grid_content = xaml_content[start_idx:end_idx]
    last_grid_end = grid_content.rfind('</Grid>') + 7
    grid_content = grid_content[:last_grid_end]
    
    # Replace in MainWindow
    replacement = f'<local:{name} x:Name="{name}" Visibility="Collapsed" Margin="0,16,0,0"/>'
    # Actually wait, let's keep it simple
    
    return grid_content

activity_grid = extract_grid('ActivityView')
transfers_grid = extract_grid('TransfersView')
devices_grid = extract_grid('DevicesView')

# Generate UserControls
def create_uc(name, content):
    uc_content = f"""<UserControl x:Class="Deskdrop.Windows.Views.{name}"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" 
             xmlns:d="http://schemas.microsoft.com/expression/blend/2008" 
             xmlns:iconPacks="http://metro.mahapps.com/winfx/xaml/iconpacks"
             xmlns:local="clr-namespace:Deskdrop.Windows.Views"
             mc:Ignorable="d" 
             d:DesignHeight="450" d:DesignWidth="800">
{content}
</UserControl>"""
    with open(f'{views_dir}/{name}.xaml', 'w') as f:
        f.write(uc_content)
        
    cs_content = f"""using System.Windows.Controls;

namespace Deskdrop.Windows.Views
{{
    public partial class {name} : UserControl
    {{
        public {name}()
        {{
            InitializeComponent();
        }}
    }}
}}
"""
    with open(f'{views_dir}/{name}.xaml.cs', 'w') as f:
        f.write(cs_content)

os.makedirs(views_dir, exist_ok=True)
create_uc('ActivityView', activity_grid)
create_uc('TransfersView', transfers_grid)
create_uc('DevicesView', devices_grid)

# We will let the replace tool handle the exact replacement in MainWindow to avoid slicing errors.
print("AppStyles and UserControls created.")
