import { capitalizeFirstLetter } from "./content.js";
import { Notification, NotificationFieldContext } from "./json";

export class NotificationHandler {
	notify_idx: number
	div_notifications: HTMLElement;

	constructor() {
		this.notify_idx = 0;

		this.div_notifications =
				document
					.getElementById("div-notifications")!;
	}

	processNotifications(notifications: Notification[]) {
        for (let notify of notifications) {
            let [message, color] = notificationTypeResolver(notify);

            this.div_notifications.innerHTML += `
                <div id="toast-${this.notify_idx}" class="toast show align-items-center ${color} border-0" role="alert" aria-live="assertive"
                    aria-atomic="true">
                    <div class="d-flex">
                        <div class="toast-body">
                            ${message}
                        </div>
                        <button id="toast-${this.notify_idx}-close-btn" type="button" class="btn-close btn-close-white me-2 m-auto" data-bs-dismiss="toast"
                            aria-label="Close"></button>
                    </div>
                </div>
            `;

            let toast: HTMLElement = document
                .getElementById(`toast-${this.notify_idx}`)!;

            // Add handler for close button.
            document
                .getElementById(`toast-${this.notify_idx}-close-btn`)!
                .addEventListener("click", (e: Event) => {
                    toast.classList.remove("show");
                    toast.classList.add("hide");
                });

            this.notify_idx += 1;

            // Cleanup old toast, limit to five max.
            let old = this.notify_idx - 5;
            if (old >= 0) {
                let toast: HTMLElement = document
                    .getElementById(`toast-${this.notify_idx}`)!;

                toast.classList.remove("show");
                toast.classList.add("hide");
            }
        }
	}
}

function notificationTypeResolver(notification: Notification): [string, string] {
    switch (notification.type) {
        case "field_verified": {
            let data = notification.value as NotificationFieldContext;
            return [
            `${capitalizeFirstLetter(data.field.type)} account "${data.field.value}" is verified. Challenge is valid.`,
            "bg-success text-light",
            ]
        }
        case "field_verification_failed": {
            let data = notification.value as NotificationFieldContext;
            return [
            `${capitalizeFirstLetter(data.field.type)} account "${data.field.value}" failed to get verified. Invalid challenge.`,
            "bg-danger text-light"
            ]
        }
        case "second_field_verified": {
            let data = notification.value as NotificationFieldContext;
            return [
            `${capitalizeFirstLetter(data.field.type)} account "${data.field.value}" is fully verified. Additional challenge is valid.`,
            "bg-success text-light"
            ]
        }
        case "second_field_verification_failed": {
            let data = notification.value as NotificationFieldContext;
            return [
            `${capitalizeFirstLetter(data.field.type)} account "${data.field.value}" failed to get verified. The additional challenge is invalid.`,
            "bg-danger text-light"
            ]
        }
        case "awaiting_second_challenge": {
            let data = notification.value as NotificationFieldContext;
            return [
            `A second challenge was sent to ${capitalizeFirstLetter(data.field.type)} account "${data.field.value}".`,
            "bg-info text-dark"
            ]
        }
        case "identity_fully_verified": {
            return [
            `<strong>Verification process completed!</strong> Judgement will be issued shortly.`,
            "bg-success text-light"
            ]
        }
        case "judgement_provided": {
        }
        default: {
            // TODO
        }
    }

    return ["TODO", "TODO"]
}