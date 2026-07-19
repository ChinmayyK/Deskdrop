import os
import re

views_dir = '/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/Views'
vms_dir = '/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/ViewModels'
os.makedirs(vms_dir, exist_ok=True)

for view_name in ['ActivityView', 'TransfersView', 'DevicesView']:
    xaml_file = f'{views_dir}/{view_name}.xaml'
    with open(xaml_file, 'r') as f:
        content = f.read()

    # Find all event handlers: EventName="Method_Name"
    # Note: MouseLeftButtonDown="ActivityFeedItem_Click", Click="BtnTransferPrimaryAction_Click", etc.
    # Replace with Command="{Binding MethodNameCommand}"
    
    # We will look for Click="..." and MouseLeftButtonDown="..."
    handlers = set(re.findall(r'Click="([^"]+)"', content))
    handlers.update(re.findall(r'MouseLeftButtonDown="([^"]+)"', content))

    for handler in handlers:
        content = content.replace(f'Click="{handler}"', f'Command="{{Binding {handler}Command}}"')
        content = content.replace(f'MouseLeftButtonDown="{handler}"', f'Command="{{Binding {handler}Command}}"')

    with open(xaml_file, 'w') as f:
        f.write(content)

    vm_name = view_name.replace('View', 'ViewModel')
    commands_code = "\n".join([f"""        public ICommand {handler}Command {{ get; }}""" for handler in handlers])

    vm_content = f"""using System.Windows.Input;

namespace Deskdrop.Windows.ViewModels
{{
    public class {vm_name}
    {{
{commands_code}
        
        public {vm_name}()
        {{
        }}
    }}
}}
"""
    with open(f'{vms_dir}/{vm_name}.cs', 'w') as f:
        f.write(vm_content)

# Update MainWindow.xaml to replace grids with UserControls
mainwindow_xaml = '/Users/chinmayk/Projects/Deskdrop/platforms/windows/Deskdrop.Windows/MainWindow.xaml'
with open(mainwindow_xaml, 'r') as f:
    mw_content = f.read()

def replace_grid(name):
    global mw_content
    start_tag = f'<Grid x:Name="{name}"'
    end_tag = '<!-- SETTINGS VIEW -->' if name == 'DevicesView' else f'<!-- {name.replace("View", "").upper()}S VIEW -->' if name == 'ActivityView' else '<!-- DEVICES VIEW -->'
    
    start_idx = mw_content.find(start_tag)
    end_idx = mw_content.find(end_tag) - 17 
    
    grid_content = mw_content[start_idx:end_idx]
    last_grid_end = grid_content.rfind('</Grid>') + 7
    full_str = mw_content[start_idx : start_idx + last_grid_end]
    
    mw_content = mw_content.replace(full_str, f'<local2:{name} x:Name="{name}" Visibility="Collapsed" Margin="0,16,0,0"/>')

replace_grid('ActivityView')
replace_grid('TransfersView')
replace_grid('DevicesView')

# Need to add local2 xmlns to MainWindow.xaml
mw_content = mw_content.replace('xmlns:local="clr-namespace:Deskdrop.Windows"', 'xmlns:local="clr-namespace:Deskdrop.Windows"\n        xmlns:local2="clr-namespace:Deskdrop.Windows.Views"')

with open(mainwindow_xaml, 'w') as f:
    f.write(mw_content)

print("Created ViewModels and updated bindings.")
