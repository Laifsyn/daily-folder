# Impresos

Sistema de creación automática de carpetas para trabajos de impresión.
Genera una estructura de directorios basada en la fecha y, bajo ciertas
condiciones, crea una subcarpeta `impresos` dentro del directorio del día.

## Instalación

El binario se compila como parte del workspace:

```bash
cargo build --release -p utils
```

El ejecutable se encontrará en `target/release/impresos.exe` (Windows) o
`target/release/impresos` (Linux/macOS).

## Uso rápido

```bash
# Modo continuo (daemon) — se queda corriendo indefinidamente
./impresos

# Modo continuo con intervalo personalizado (cada 60 segundos)
./impresos --interval 60

# Ejecutar una sola vez para hoy y salir (útil para cron / task scheduler)
./impresos --once

# Fecha específica (administrativo, una sola ejecución)
./impresos --date 2025-03-07

# Rango de fechas — backfill administrativo
./impresos --start 2025-03-01 --end 2025-03-31

# Especificar archivo de configuración
./impresos --settings-file ./mi_config.toml

# Sobrescribir el directorio raíz
./impresos --root-directory D:\trabajos
```

## Opciones CLI

| Opción | Variable de entorno | Descripción |
|---|---|---|
| `--root-directory` | `IMPRESOS_ROOT_DIRECTORY` | Directorio raíz donde se crea la estructura de fechas. Sobrescribe lo definido en el `.toml`. |
| `--settings-file` | `IMPRESOS_SETTINGS_FILE` | Ruta al archivo `.toml` de configuración. Por defecto: `./.settings/impresos.toml`. |
| `--date` | `IMPRESOS_DATE` | **(Admin)** Fecha única a procesar (`YYYY-MM-DD`). El programa sale tras procesarla. |
| `--start` | `IMPRESOS_START` | **(Admin)** Inicio de rango de backfill (`YYYY-MM-DD`). Usar con `--end`. |
| `--end` | `IMPRESOS_END` | **(Admin)** Fin de rango de backfill (`YYYY-MM-DD`). Usar con `--start`. |
| `--once` | `IMPRESOS_ONCE` | Procesa el día actual una vez y sale. Útil para cron / tareas programadas. |
| `--interval` | `IMPRESOS_INTERVAL` | Segundos entre chequeos en modo continuo. Por defecto: `300` (5 min). |

## Modos de operación

| Argumentos | Comportamiento |
|---|---|
| *(ninguno)* | **Modo continuo (daemon).** Procesa el día actual y repite cada `--interval` segundos indefinidamente. |
| `--once` | Procesa el día actual una vez y sale. |
| `--date` | Una sola ejecución para la fecha indicada, luego sale. |
| `--start` + `--end` | Una sola ejecución para el rango de fechas (backfill administrativo), luego sale. |

## Archivo de configuración

Al ejecutar el programa por primera vez, se crea automáticamente
`./.settings/impresos.toml` con valores por defecto y comentarios
explicativos.

### Campos

```toml
# Directorio raíz. Puede ser relativo o absoluto (ej. "D:\")
root_directory = "./"

# Template con especificadores de Chrono.
# %Y = año 4 dígitos   %m = mes 2 dígitos
# %d = día 2 dígitos   %B = nombre del mes (de la tabla [month_names])
create_path = "%Y/%m %B/%d"

# Extensiones que disparan la carpeta "impresos"
extension_trigger_impresos = ["pdf", "png", "_tiff"]

# Si un día supera esta cantidad de archivos, se crea "impresos"
max_files_before_trigger = 50

# Nombre de la subcarpeta que se crea
impresos_folder_name = "impresos"
```

### Tabla de nombres de meses

Opcional. Si el sistema operativo maneja bien la localización, se puede dejar
vacía. Por defecto se rellena con nombres en español para compatibilidad con
Windows 7.

```toml
[month_names]
"01" = "enero"
"02" = "febrero"
# ...
"12" = "diciembre"
```

## Lógica de dominio

1. Dado un `root_directory` y una fecha, el programa genera una ruta como:
   ```
   ./2025/03 marzo/15/
   ```

2. Verifica si el directorio del día cumple **al menos una** de estas condiciones:
   - Contiene más de `max_files_before_trigger` archivos.
   - Contiene al menos un archivo con extensión listada en
     `extension_trigger_impresos`.

3. Si se cumple, crea la subcarpeta:
   ```
   ./2025/03 marzo/15/impresos/
   ```

4. En **modo continuo**, este chequeo se repite cada `--interval` segundos.
   La operación es **idempotente**: si la carpeta `impresos` ya existe, no
   se duplica ni se modifica.

## Logs

Los logs se escriben a dos destinos simultáneamente:

- **Consola** (`stderr`): filtrados por variable de entorno `RUST_LOG`
  (por defecto: `impresos=info`).
- **Archivo**: rolling diario en `./.logs/impresos/impresos.log.YYYY-MM-DD`.

Para subir la verbosidad a depuración:

```bash
RUST_LOG=impresos=debug ./impresos
```

## Estructura del código

```
src/impresos/
├── mod.rs        — módulo raíz, re-exports
├── error.rs      — ImpresosError (estático, thiserror)
├── settings.rs   — Settings, defaults, carga/creación del .toml
├── template.rs   — expand_template (Chrono + month_names)
├── ops.rs        — should_create_impresos, run_for_date, run_for_date_range
├── app.rs        — CLI, logging, spawn_blocking, modo continuo (capa de aplicación)
└── readme.md     — este archivo
```

La separación sigue el principio de que la **capa de dominio** no conoce
`tokio`, `color_eyre`, ni `tracing` — solo maneja errores estáticos. La
**capa de aplicación** (`app.rs`) orquesta las llamadas bloqueantes sobre
`spawn_blocking` y convierte los errores a `color_eyre::Report`.
