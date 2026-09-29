import { insert, t, type Dictionary } from "intlayer";

const content = {
  key: "favorites",
  content: {
    noFavorites: t({ en: "No favorites yet", fr: "Pas encore de favoris", de: "Noch keine Favoriten", es: "No hay favoritos aún", ru: "Нет избранного" }),
    noFavoritesHint: t({ en: "Press F on any server in the browser to add it", fr: "Appuyez sur F sur un serveur dans le navigateur pour l'ajouter", de: "Drücke F auf einem Server im Browser, um ihn hinzuzufügen", es: "Presiona F en cualquier servidor del navegador para agregarlo", ru: "Нажмите F на любом сервере в браузере, чтобы добавить его" }),
    browseServers: t({ en: "Browse Servers", fr: "Parcourir les serveurs", de: "Server durchsuchen", es: "Explorar servidores", ru: "Обзор серверов" }),
    serverOffline: t({ en: "OFFLINE", fr: "HORS LIGNE", de: "OFFLINE", es: "DESCONECTADO", ru: "ОФФЛАЙН" }),
    serverOfflineHint: t({ en: "Server not found in the current server list — it may be offline, or try refreshing the server list", fr: "Serveur non trouvé — peut être hors ligne, essayez d'actualiser", de: "Server nicht in aktueller Liste gefunden — möglicherweise offline oder Serverliste aktualisieren", es: "Servidor no encontrado en la lista actual — puede estar desconectado, o intenta actualizar la lista de servidores", ru: "Сервер не найден в текущем списке — возможно, не в сети, или попробуйте обновить список серверов" }),
    closeDetails: t({ en: "Close details", fr: "Fermer les détails", de: "Details schließen", es: "Cerrar detalles", ru: "Закрыть детали" }),
    liveDetails: t({ en: "Live server details", fr: "Détails du serveur en direct", de: "Live-Server-Details", es: "Detalles del servidor en vivo", ru: "Детали сервера" }),
    querying: t({ en: "Querying…", fr: "Interrogation…", de: "Abfrage läuft…", es: "Consultando…", ru: "Запрос…" }),
    close: t({ en: "Close", fr: "Fermer", de: "Schließen", es: "Cerrar", ru: "Закрыть" }),
    onlineCount: insert(t({ en: "Online ({{count}})", fr: "En ligne ({{count}})", de: "Online ({{count}})", es: "En línea ({{count}})", ru: "Онлайн ({{count}})" })),
    refreshA2s: t({ en: "Refresh A2S", fr: "Actualiser A2S", de: "A2S aktualisieren", es: "Actualizar A2S", ru: "Обновить A2S" }),
    refreshA2sTitle: t({ en: "Re-query live server info via A2S protocol", fr: "Ré-interroger les infos serveur via protocole A2S", de: "Live-Server-Info per A2S-Protokoll erneut abfragen", es: "Reconsultar info del servidor en vivo vía protocolo A2S", ru: "Повторный запрос информации о сервере через A2S протокол" }),
    added: insert(t({ en: "Added {{name}} to favorites", fr: "{{name}} ajouté aux favoris", de: "{{name}} zu Favoriten hinzugefügt", es: "Añadido {{name}} a favoritos", ru: "Добавлено {{name}} в избранное" })),
    removeTitle: t({ en: "Remove Favorite", fr: "Retirer des favoris", de: "Favorit entfernen", es: "Quitar favorito", ru: "Удаление из избранного" }),
    removeMessage: insert(t({ en: "Remove '{{name}}' from favorites?", fr: "Retirer '{{name}}' des favoris ?", de: "'{{name}}' aus Favoriten entfernen?", es: "¿Quitar '{{name}}' de favoritos?", ru: "Удалить '{{name}}' из избранного?" })),
    removed: t({ en: "Removed from favorites", fr: "Retiré des favoris", de: "Aus Favoriten entfernt", es: "Quitado de favoritos", ru: "Удалено из избранного" }),
    ipExcluded: insert(t({ en: "{{ip}} excluded from server list", fr: "{{ip}} exclu de la liste des serveurs", de: "{{ip}} von Serverliste ausgeschlossen", es: "{{ip}} excluida de la lista de servidores", ru: "{{ip}} исключён из списка серверов" })),
    ipExcludeFailed: t({ en: "Failed to exclude IP", fr: "Échec de l'exclusion de l'IP", de: "IP konnte nicht ausgeschlossen werden", es: "Error al excluir IP", ru: "Не удалось исключить IP" }),
    ipUnexcluded: insert(t({ en: "{{ip}} removed from exclusions", fr: "{{ip}} retiré des exclusions", de: "{{ip}} von Ausschlüssen entfernt", es: "{{ip}} quitada de exclusiones", ru: "{{ip}} убран из исключений" })),
  },
} satisfies Dictionary;

export default content;
