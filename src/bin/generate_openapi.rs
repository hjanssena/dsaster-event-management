use eventManagement_api::openapi::ApiDoc;
use std::fs::File;
use std::io::Write;
use utoipa::OpenApi;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Genera la especificación OpenAPI en formato YAML desde utoipa
    let yaml_output = ApiDoc::openapi().to_yaml()?;

    // 2. Guarda el archivo openapi.yml en la raíz del proyecto
    let mut file = File::create("openapi.yml")?;
    file.write_all(yaml_output.as_bytes())?;

    println!("openapi.yml file successfully generated in the project root");
    Ok(())
}
