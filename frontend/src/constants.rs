pub const ERROR_TITLE: &str = "Error";
pub const ERROR_SELECT_FILE: &str = "Please select at least one file to upload.";
pub const ERROR_NETWORK: &str = "Failed to upload files (network error)";
pub const LABEL_SET_LATITUDE: &str = "Set latitude";
pub const LABEL_SET_LONGITUDE: &str = "Set longitude";
pub const LABEL_SET_DATE_TIME: &str = "Set date and time";
pub const LABEL_SELECT_FILES: &str = "Select files to upload";
pub const BUTTON_SUBMIT: &str = "Submit";
pub const BUTTON_CANCEL: &str = "Cancel";

pub const LATITUDE_MIN: &str = "-90";
pub const LATITUDE_MAX: &str = "90";
pub const LONGITUDE_MIN: &str = "-180";
pub const LONGITUDE_MAX: &str = "180";

// Home
pub const MAP_TITLE: &str = "Map";
pub const MAP_INITIAL_LAT: f64 = 51.505;
pub const MAP_INITIAL_LNG: f64 = -0.09;
pub const MAP_INITIAL_ZOOM: f64 = 3.0;
pub const TILE_LAYER_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
pub const TILE_LAYER_ATTRIBUTION: &str =
	"&copy; <a href=\"https://www.openstreetmap.org/copyright\">OpenStreetMap</a> contributors";

// Sign In
pub const TITLE_SIGN_IN: &str = "Sign In";
pub const LABEL_EMAIL: &str = "Email";
pub const LABEL_PASSWORD: &str = "Password";
pub const BUTTON_SIGN_IN: &str = "Sign In";
pub const BUTTON_FORGOT_PASSWORD: &str = "Forgot Password?";
pub const LINK_REGISTER: &str = "Don't have an account? Register";
pub const MSG_ENTER_EMAIL_RESET: &str = "Please enter your email address to reset password";
pub const MSG_RESET_EMAIL_SENT: &str = "Password reset email sent";

// Register
pub const TITLE_REGISTER: &str = "Register";
pub const LABEL_CONFIRM_PASSWORD: &str = "Confirm Password";
pub const BUTTON_REGISTER: &str = "Register";
pub const MSG_PASSWORDS_DO_NOT_MATCH: &str = "Passwords do not match";

// Account
pub const TITLE_ACCOUNT_SETTINGS: &str = "Account Settings";
pub const TITLE_DEFAULT_PUBLICITY: &str = "Default Publicity";
pub const LABEL_DEFAULT_PUBLICITY: &str = "Default Publicity for New Objects";
pub const OPTION_PUBLIC: &str = "Public";
pub const OPTION_PRIVATE: &str = "Private";
pub const TITLE_CHANGE_EMAIL: &str = "Change Email";
pub const LABEL_NEW_EMAIL: &str = "New Email";
pub const BUTTON_UPDATE_EMAIL: &str = "Update Email";
pub const TITLE_CHANGE_PASSWORD: &str = "Change Password";
pub const LABEL_OLD_PASSWORD: &str = "Old Password";
pub const LABEL_NEW_PASSWORD: &str = "New Password";
pub const LABEL_CONFIRM_NEW_PASSWORD: &str = "Confirm New Password";
pub const BUTTON_UPDATE_PASSWORD: &str = "Update Password";
pub const MSG_EMAIL_UPDATED: &str = "Email updated successfully";
pub const MSG_PASSWORD_UPDATED: &str = "Password updated successfully";
pub const MSG_PUBLICITY_UPDATED: &str = "Default publicity updated successfully";
pub const MSG_NEW_PASSWORDS_DO_NOT_MATCH: &str = "New passwords do not match";

// Admin Users
pub const TITLE_USERS: &str = "Users";
pub const HEADER_ID: &str = "ID";
pub const HEADER_EMAIL: &str = "Email";
pub const HEADER_ROLE: &str = "Role";
pub const HEADER_CREATED_AT: &str = "Created At";
pub const HEADER_ACTIONS: &str = "Actions";
pub const BUTTON_SAVE: &str = "Save";
pub const BUTTON_RESET_PASSWORD: &str = "Reset Password";
pub const OPTION_USER: &str = "User";
pub const OPTION_ADMIN: &str = "Admin";
pub const LOADING_TEXT: &str = "Loading...";

// Reset Password
pub const TITLE_RESET_PASSWORD: &str = "Reset Password";
pub const MSG_INVALID_TOKEN: &str = "Invalid token";
pub const MSG_RESET_SUCCESS: &str = "Password reset successfully. Redirecting to sign in...";

// Objects
pub const TITLE_OBJECTS: &str = "Objects";
pub const BUTTON_ADD_OBJECT: &str = "Add Object";
pub const TITLE_ADD_OBJECT: &str = "Add Object";
pub const TITLE_EDIT_OBJECT: &str = "Edit Object";
pub const BUTTON_CLOSE: &str = "Close";

// Objects Table
pub const BUTTON_DELETE_SELECTED: &str = "Delete selected";
pub const MSG_DELETE_SUCCESS: &str = "Deleted objects successfully";
pub const MSG_DELETE_FAILED: &str = "Failed to delete objects";
pub const HEADER_SELECT: &str = "Select";
pub const HEADER_NAME: &str = "Name";
pub const HEADER_MADE_ON: &str = "Made On";
pub const HEADER_LOCATION: &str = "Location";
pub const HEADER_VIEW: &str = "View";
pub const HEADER_CONTENT_TYPE: &str = "Content Type";
pub const MSG_CONFIRM_DELETE: &str = "Are you sure you want to delete ";
pub const BUTTON_YES: &str = "Yes";
pub const BUTTON_NO: &str = "No";

// Errors
pub const TITLE_404: &str = "Uh oh!";
pub const MSG_404: &str = "We couldn't find that page!";
pub const TITLE_403: &str = "403 Forbidden";
pub const MSG_403: &str = "You do not have permission to view this page.";

// Header
pub const LINK_MAP: &str = "Map";
pub const LINK_OBJECTS: &str = "Objects";
pub const LINK_ACCOUNT: &str = "Account";
pub const LINK_USERS: &str = "Users";
pub const BUTTON_LOGOUT: &str = "Log Out";
pub const LINK_SIGN_IN: &str = "Sign In";
pub const ARIA_CLOSE_MENU: &str = "Close menu";
pub const ARIA_OPEN_MENU: &str = "Open menu";

// Edit Object Form
pub const TITLE_INVALID_EMAILS: &str = "Invalid Emails";
pub const MSG_INVALID_EMAILS: &str = "Invalid email addresses: ";
pub const TITLE_SUCCESS: &str = "Success";
pub const MSG_OBJECT_UPDATED: &str = "Object updated successfully";
pub const TITLE_WARNING: &str = "Warning";
pub const MSG_MISSING_USERS: &str = "The following users were not found: ";
pub const MSG_UPDATE_FAILED: &str = "Failed to update object: ";
pub const LABEL_NAME: &str = "Name";
pub const LABEL_PUBLICITY: &str = "Publicity";
pub const OPTION_DEFAULT: &str = "Default";
pub const OPTION_SELECTED_USERS: &str = "Selected Users";
pub const LABEL_ALLOWED_USERS: &str = "Allowed Users (comma separated emails)";
pub const PLACEHOLDER_ALLOWED_USERS: &str = "user1@example.com, user2@example.com";
pub const MSG_ERROR_LOADING_OBJECT: &str = "Error loading object: ";

pub const HEADER_HEIGHT: f64 = 100.0;
pub const HEADER_LAYER_CLASSES: &str = "hide-on-scroll inset-0 h-100px w-dvw translate-y-[--hide-on-scroll-translate-y] group-[:not(.scrolling)]/page:transition-all";

// Config
pub const DEFAULT_FRONTEND_URL: &str = "http://127.0.0.1:3000";
pub const CONFIG_FILE_PATH: &str = "/config.json";
pub const MSG_FAILED_TO_PARSE_CONFIG: &str = "Failed to parse config";
pub const MSG_FAILED_TO_FETCH_CONFIG: &str = "Failed to fetch config";
pub const MSG_LOADING_CONFIG: &str = "Loading config...";
pub const TITLE_FAILED_LOAD_CONFIG: &str = "Failed to load configuration";
pub const MSG_CHECK_CONSOLE: &str = "Please check the console for more details.";
