import { insert, t, type Dictionary } from "intlayer";

// The words of the window's frame: the rail, its groups, the status bar.
const content = {
  key: "nav",
  content: {
    groupPlay: t({ en: "Play", fr: "Jouer", de: "Spielen", es: "Jugar", ru: "Игра" }),
    groupGear: t({ en: "Gear", fr: "Équipement", de: "Ausrüstung", es: "Equipo", ru: "Снаряжение" }),
    groupIntel: t({ en: "Intel", fr: "Infos", de: "Infos", es: "Info", ru: "Сводки" }),
    servers: t({ en: "Servers", fr: "Serveurs", de: "Server", es: "Servidores", ru: "Серверы" }),
    favorites: t({ en: "Favorites", fr: "Favoris", de: "Favoriten", es: "Favoritos", ru: "Избранное" }),
    history: t({ en: "History", fr: "Historique", de: "Verlauf", es: "Historial", ru: "История" }),
    connect: t({
      en: "Direct connect",
      fr: "Connexion directe",
      de: "Direktverbindung",
      es: "Conexión directa",
      ru: "Прямое подключение",
    }),
    offline: t({ en: "Offline", fr: "Hors ligne", de: "Offline", es: "Sin conexión", ru: "Офлайн" }),
    mods: t({ en: "Mods", fr: "Mods", de: "Mods", es: "Mods", ru: "Моды" }),
    options: t({
      en: "Launch options",
      fr: "Options de lancement",
      de: "Startoptionen",
      es: "Opciones de inicio",
      ru: "Параметры запуска",
    }),
    news: t({ en: "News", fr: "Actualités", de: "Neuigkeiten", es: "Noticias", ru: "Новости" }),
    settings: t({ en: "Settings", fr: "Réglages", de: "Einstellungen", es: "Ajustes", ru: "Настройки" }),
    about: t({ en: "About", fr: "À propos", de: "Über", es: "Acerca de", ru: "О программе" }),
    collapse: t({
      en: "Collapse the menu",
      fr: "Réduire le menu",
      de: "Menü einklappen",
      es: "Contraer el menú",
      ru: "Свернуть меню",
    }),
    expand: t({
      en: "Expand the menu",
      fr: "Déplier le menu",
      de: "Menü ausklappen",
      es: "Expandir el menú",
      ru: "Развернуть меню",
    }),
    rejoin: t({ en: "Rejoin", fr: "Rejoindre", de: "Erneut beitreten", es: "Volver a unirse", ru: "Вернуться" }),
    lastPlayed: t({
      en: "Last played",
      fr: "Dernière partie",
      de: "Zuletzt gespielt",
      es: "Última partida",
      ru: "Последняя игра",
    }),
    notListed: t({
      en: "Not in the current list",
      fr: "Absent de la liste actuelle",
      de: "Nicht in der aktuellen Liste",
      es: "No está en la lista actual",
      ru: "Нет в текущем списке",
    }),
    scanning: insert(
      t({
        en: "Pinging {{done}} / {{total}}",
        fr: "Ping {{done}} / {{total}}",
        de: "Ping {{done}} / {{total}}",
        es: "Ping {{done}} / {{total}}",
        ru: "Пинг {{done}} / {{total}}",
      }),
    ),
    scanPaused: t({ en: "Ping paused", fr: "Ping en pause", de: "Ping pausiert", es: "Ping en pausa", ru: "Пинг на паузе" }),
    pauseScan: t({
      en: "Pause the ping scan",
      fr: "Mettre le ping en pause",
      de: "Ping-Scan pausieren",
      es: "Pausar el escaneo de ping",
      ru: "Приостановить пинг",
    }),
    resumeScan: t({
      en: "Resume the ping scan",
      fr: "Reprendre le ping",
      de: "Ping-Scan fortsetzen",
      es: "Reanudar el escaneo de ping",
      ru: "Продолжить пинг",
    }),
    listAge: insert(
      t({
        en: "List updated {{age}}",
        fr: "Liste mise à jour {{age}}",
        de: "Liste aktualisiert {{age}}",
        es: "Lista actualizada {{age}}",
        ru: "Список обновлён {{age}}",
      }),
    ),
    listStale: t({
      en: "The server list is getting old",
      fr: "La liste des serveurs commence à dater",
      de: "Die Serverliste ist veraltet",
      es: "La lista de servidores está desactualizada",
      ru: "Список серверов устарел",
    }),
    modOpRunning: insert(
      t({
        en: "SteamCMD: {{current}} of {{total}}",
        fr: "SteamCMD : {{current}} sur {{total}}",
        de: "SteamCMD: {{current}} von {{total}}",
        es: "SteamCMD: {{current}} de {{total}}",
        ru: "SteamCMD: {{current}} из {{total}}",
      }),
    ),
    showProgress: t({
      en: "Show the progress",
      fr: "Afficher la progression",
      de: "Fortschritt anzeigen",
      es: "Mostrar el progreso",
      ru: "Показать прогресс",
    }),
    version: insert(t({ en: "v{{version}}", fr: "v{{version}}", de: "v{{version}}", es: "v{{version}}", ru: "v{{version}}" })),
    shortcuts: t({
      en: "Keyboard shortcuts",
      fr: "Raccourcis clavier",
      de: "Tastenkürzel",
      es: "Atajos de teclado",
      ru: "Горячие клавиши",
    }),
  },
} satisfies Dictionary;

export default content;
