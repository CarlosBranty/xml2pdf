# Seguridad

## Reportar una vulnerabilidad

No abras un reporte público. Usa el reporte privado de GitHub: en la pestaña **Security** del repositorio, elige **Report a vulnerability**.

Incluye, en lo posible:

- Qué pieza afecta: web, CLI o escritorio.
- Los pasos o el archivo que la reproduce, sin datos reales de comprobantes.
- El efecto que observaste.

Recibirás respuesta por ese mismo canal. Pedimos no divulgar el problema hasta que haya una corrección publicada.

## Versiones con soporte

El proyecto está en la serie 0.x: las correcciones de seguridad se aplican solo a la última versión.

## Qué se considera una vulnerabilidad

xml2pdf procesa archivos que el usuario recibe de terceros, así que un XML o ZIP malicioso es el caso principal.

| Dentro del alcance | Ejemplo |
|--------------------|---------|
| Un XML o ZIP que agota la memoria o cuelga el programa | Un ZIP que se expande sin límite |
| Un XML que escribe fuera de la carpeta de salida | Un número de comprobante con `../` |
| Contenido del XML que se ejecuta como código | Texto que la plantilla interpreta como instrucciones |
| Que la web envíe el comprobante fuera del equipo | Una petición de red con datos del XML |
| Que el escritorio ejecute acciones no pedidas | Un comando disponible para contenido no confiable |

Queda fuera del alcance lo que requiere que el atacante ya controle el equipo del usuario, y los fallos de una plantilla Typst escrita por el propio usuario.
