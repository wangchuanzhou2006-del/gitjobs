const REVIEW_QUEUE_PATH = "/dashboard/moderator/talent/review";

document.addEventListener("talent-review-refresh", (event) => {
  const destination = event?.detail?.destination;
  if (destination === "detail") {
    window.location.reload();
  } else if (destination === "queue") {
    window.location.assign(REVIEW_QUEUE_PATH);
  }
});
