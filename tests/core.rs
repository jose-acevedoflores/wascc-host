use reqwest;
use std::error::Error;
use wascc_host::Host;

pub(crate) fn stock_host() -> Result<(), Box<dyn Error>> {
    let host = crate::common::gen_stock_host(9090)?;
    assert_eq!(2, host.actors().len());
    if let Some(ref claims) =
        host.claims_for_actor("MB4OLDIC3TCZ4Q4TGGOVAZC43VXFE2JQVRAXQMQFXUCREOOFEKOKZTY2")
    {
        let md = claims.metadata.as_ref().unwrap();
        assert!(md
            .caps
            .as_ref()
            .unwrap()
            .contains(&"wascc:http_server".to_string()));
    }

    std::thread::sleep(::std::time::Duration::from_millis(500));

    let resp = reqwest::blocking::get("http://localhost:9090")?;
    assert!(resp.status().is_success());
    assert_eq!(resp.text()?,
        "{\"method\":\"GET\",\"path\":\"/\",\"query_string\":\"\",\"headers\":{\"accept\":\"*/*\",\"host\":\"localhost:9090\"},\"body\":[]}"
    );
    host.shutdown()?;
    std::thread::sleep(::std::time::Duration::from_millis(500));
    Ok(())
}

pub(crate) fn kv_host() -> Result<(), Box<dyn Error>> {
    use redis::Commands;

    let host = crate::common::gen_kvcounter_host(8083, Host::new())?;
    std::thread::sleep(::std::time::Duration::from_millis(100));
    let key = uuid::Uuid::new_v4().to_string();
    let rkey = format!(":{}", key); // the kv wasm logic does a replace on '/' with ':'
    let url = format!("http://localhost:8083/{}", key);
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut con = client.get_connection()?;

    let mut resp = reqwest::blocking::get(&url)?;
    assert!(resp.status().is_success());
    reqwest::blocking::get(&url)?;
    resp = reqwest::blocking::get(&url)?; // counter should be at 3 now
    assert!(resp.status().is_success());
    assert_eq!(resp.text()?, "{\"counter\":3}");
    host.shutdown()?;

    let _: () = con.del(&rkey)?;
    Ok(())
}
