# Registro de cambios

Los cambios notables de cada versión se anotan aquí. El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el proyecto usa [versionado semántico](https://semver.org/lang/es/).

## [Sin publicar]

### Añadido

- Lectura de facturas (01), boletas de venta (03), notas de crédito (07) y notas de débito (08) en XML UBL 2.1, sueltas o dentro de un ZIP.
- Generación de PDF en A4, A5, Carta, ticket de 80 mm y ticket de 58 mm.
- Línea de comandos `xml2pdf` con los comandos `convert`, `watch`, `printers`, `template` y `data`.
- Aplicación web que convierte en el navegador, con vista previa en todos los formatos y conversión por lotes.
- Aplicación de escritorio con la misma interfaz e impresión automática de una carpeta vigilada.
- Diseñador de formato por bloques, con estilos listos y exportación a un archivo que el CLI acepta.
- Personalización con logo, texto al pie, código QR opcional y plantillas Typst propias.
- Imagen Docker y workflows para desplegar la web y publicar versiones.
