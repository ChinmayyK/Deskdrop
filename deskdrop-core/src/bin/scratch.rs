use deskdrop_core::ipc::{client::IpcClient, IpcRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = IpcClient::connect().await?;
    println!("Accepting pairing...");
    let resp = client
        .request(&IpcRequest::RespondToPairing {
            device_id: "1537846c-64ba-5640-a3ea-59fb129addcb".to_string(),
            accepted: true,
        })
        .await?;
    println!("Response: {:?}", resp);
    Ok(())
}
