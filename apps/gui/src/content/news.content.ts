import { insert, t, type Dictionary } from "intlayer";

const content = {
  key: "news",
  content: {
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
