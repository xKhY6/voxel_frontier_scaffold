use std::process::Command;

// Test de fumée : le binaire lancé avec `--check` doit afficher son message et sortir sans ouvrir de fenêtre.
// Sans `--check`, le binaire ouvre une fenêtre et bloque : ce test ne doit pas le lancer ainsi.
#[test]
fn smoke() {
    let output = Command::new(env!("CARGO_BIN_EXE_voxel_frontier"))
        .arg("--check")
        .output()
        .expect("impossible de lancer le binaire voxel_frontier");

    assert!(
        output.status.success(),
        "échec du binaire : {:?}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Voxel Frontier bootstrap"),
        "sortie inattendue : {stdout}"
    );
}
