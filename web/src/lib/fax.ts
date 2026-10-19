export interface Fax {
	id: string;
	owner_id: string;
	direction: string;
	state: string;
	source: string;
	destination: string;
	sender_email: string;
	pages: number;
	result: string;
	created_at: string;
	number_id: string | null;
	attempts: number;
}
export interface FaxEndpoint {
	id: string;
	name: string;
	owner_id: string;
	send_users: string[];
	receive_users: string[];
	reception_email: string;
	enabled: boolean;
}
export interface FaxWhitelist {
	email: string;
	user_id: string;
	enabled: boolean;
}
export interface FaxConfig {
	enabled: boolean;
	global_number_id: string | null;
	always_global: boolean;
	header: string;
	footer: string;
	retention_days: number;
	imap_enabled: boolean;
	imap_host: string;
	imap_port: number;
	imap_username: string;
	imap_folder: string;
	faxin_address: string;
	trusted_mailbox: boolean;
	trunk_modes: Record<string, string>;
}
