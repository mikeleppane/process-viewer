# Simple htop like process viewer on browser

## How to Use 

### Server
Run the following command in the server project root (make sure rust is installed). This is launch the api server.
The default port is 7070. This can be changed by specifying env variable PORT. 

```shell
cargo run --release
```
#### Endpoints

* GET /api/cpus 
  * { cpu_usage: f32,  
      frequency: u64,  
      vendor_id: String,  
      brand: String,  
    }
* GET /api/memory  
  * { total_memory: String,  
      used_memory: String,  
      total_swap: String,  
      used_swap: String, 
    }
* WS /realtime/cpus 
* WS /realtime/memory 
* GET /api/health 
  * 200 "Ok" 

### Client



## Used technologies
* Server: Rust + Axum for API development
* Client: React + Typescript + Vite