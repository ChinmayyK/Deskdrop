use default_net::get_default_interface;

fn main() {
    match get_default_interface() {
        Ok(interface) => {
            println!("Interface: {}", interface.name);
            if let Some(gateway) = interface.gateway {
                println!("Gateway IP: {}", gateway.ip_addr);
            } else {
                println!("No gateway found");
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}
