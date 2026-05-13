Haceme una app en Rust que sirva por http y https (si se le pasa --cert=file ) el directorio actual o el directorio indicado como primer parámetro posicional.
Usar Tokio para el manejo de tareas asíncronas.
Que acepte --port=<puerto> y --port-ssl=<puerto> y por defecto use 4701 y 4801.  Si está usando los puertos por defecto (si no se recibió argumento de qué puertos usar) que vaya intentando con puertos más altos de uno en uno hasta encontrar puertos libres.
Si se le pasa --help debe mostrar la ayuda.
Al ejecutarse debe mostrar una TUI con colores y paneles y si se ejecuta con --web-ui los mismos datos deben mostrarse en una web-ui que debe abrirse automáticamente (definirle y buscarle un puerto también, a menos que se le pase --port-gui=<puerto>). 
Ambas UI deben mostrar: los IPs y puertos en formato de links clickeables (por defecto http://127.0.0.1:4701 pero tambien en las otras interfaces de red) , y reportar cúantos clientes conectados hay, la lista scrolleable de clientes con el mouse (si es posible), lista de archivos en proceso de descarga (y por quien), y uptime del servicio.
El usuario debe ver la lista de archivos y carpetas en la carpeta servida (el DocumentRoot) con un diseño agradable que use los colores de fondo y fonts de https://ip1.cc/  (naranja, celeste, verde, blanco, y ese color de fondo).
Que como server sea seguro (que no permita que el usuario navegue carpetas fuera del DocumentRoot señalado).
Si una carpeta contiene un index.html, que el contenido de ese archivo reemplace el renderizado de la lista de archivos de esa carpeta.
Antes de comenzar escribí un .md con todo este requisito, y haceme todas las preguntas necesarias.  
