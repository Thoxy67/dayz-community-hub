import { insert, t, type Dictionary } from "intlayer";

const content = {
  key: "news",
  content: {
    openAllOnSite: t({
      en: "Open every article on dayz.com",
      fr: "Ouvrir tous les articles sur dayz.com",
      de: "Alle Artikel auf dayz.com öffnen",
      es: "Abrir todos los artículos en dayz.com",
      ru: "Открыть все статьи на dayz.com",
    }),
    articles: insert(t({ en: "{{count}} articles", fr: "{{count}} articles", de: "{{count}} Artikel", es: "{{count}} artículos", ru: "Статей: {{count}}" })),
    allCategories: t({ en: "All", fr: "Tout", de: "Alle", es: "Todo", ru: "Все" }),
    readTime: insert(t({ en: "{{minutes}} min read", fr: "{{minutes}} min de lecture", de: "{{minutes}} Min. Lesezeit", es: "{{minutes}} min de lectura", ru: "{{minutes}} мин чтения" })),
    by: insert(t({ en: "By {{author}}", fr: "Par {{author}}", de: "Von {{author}}", es: "Por {{author}}", ru: "Автор: {{author}}" })),
    loadFailedTitle: t({ en: "The news could not be loaded", fr: "Impossible de charger les actualités", de: "Die Neuigkeiten konnten nicht geladen werden", es: "No se pudieron cargar las noticias", ru: "Не удалось загрузить новости" }),
    loadFailedHint: t({
      en: "dayz.com did not answer or refused the request. Check the connection and try again.",
      fr: "dayz.com n'a pas répondu ou a refusé la requête. Vérifiez la connexion et réessayez.",
      de: "dayz.com hat nicht geantwortet oder die Anfrage abgelehnt. Verbindung prüfen und erneut versuchen.",
      es: "dayz.com no respondió o rechazó la solicitud. Comprueba la conexión e inténtalo de nuevo.",
      ru: "dayz.com не ответил или отклонил запрос. Проверьте подключение и повторите.",
    }),
    retry: t({ en: "Try again", fr: "Réessayer", de: "Erneut versuchen", es: "Reintentar", ru: "Повторить" }),
    empty: t({ en: "No articles yet", fr: "Aucun article pour l'instant", de: "Noch keine Artikel", es: "Aún no hay artículos", ru: "Пока нет статей" }),
    latest: t({ en: "Latest", fr: "Dernier", de: "Neu", es: "Nuevo", ru: "Новое" }),
    zoomImage: t({ en: "Enlarge the image", fr: "Agrandir l'image", de: "Bild vergrößern", es: "Ampliar la imagen", ru: "Увеличить изображение" }),
    title: t({ en: "DayZ News", fr: "Actualités DayZ", de: "DayZ Neuigkeiten", es: "Noticias de DayZ", ru: "Новости DayZ" }),
    refresh: t({ en: "Refresh", fr: "Actualiser", de: "Aktualisieren", es: "Actualizar", ru: "Обновить" }),
    openBrowser: t({ en: "Open in browser", fr: "Ouvrir dans le navigateur", de: "Im Browser öffnen", es: "Abrir en navegador", ru: "Открыть в браузере" }),
    noContent: t({ en: "No content available.", fr: "Aucun contenu disponible.", de: "Kein Inhalt verfügbar.", es: "Sin contenido disponible.", ru: "Нет контента." }),
    selectArticle: t({ en: "Select an article to read", fr: "Sélectionnez un article à lire", de: "Artikel zum Lesen auswählen", es: "Selecciona un artículo para leer", ru: "Выберите статью для чтения" }),
    close: t({ en: "Close", fr: "Fermer", de: "Schließen", es: "Cerrar", ru: "Закрыть" }),
    fetchFailed: insert(t({ en: "News fetch failed: {{error}}", fr: "Échec de la récupération des actualités : {{error}}", de: "Abrufen der Nachrichten fehlgeschlagen: {{error}}", es: "Error al obtener noticias: {{error}}", ru: "Ошибка загрузки новостей: {{error}}" })),
  },
} satisfies Dictionary;

export default content;
